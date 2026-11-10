//! Point d'entrée en ligne de commande.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use std::rc::Rc;

use chrono::Local;

use crate::adapters::channel::telegram::Telegram;
use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::java_rcon::{DELAI, JavaRcon};
use crate::adapters::game::rcon::ErreurRcon;
use crate::adapters::llm::creer as creer_llm;
use crate::alea::Alea;
use crate::config::{Config, charger_config};
use crate::games::atelier::{self, ErreurAtelier};
use crate::games::coffre_du_soir::{CoffreDuSoir, Notificateur, NotificateurConsole};
use crate::games::paisible::ModePaisible;
use crate::games::pixel::Pixel;
use crate::orchestrateur::{
    CommandesParents, ErreurFile, INTERVALLE, NotificateurCanal, Orchestrateur, mettre_en_file,
    servir,
};
use crate::storage::sqlite::{Depot, ErreurStockage, maintenant};
use crate::{VERSION, datapack, photo};

#[derive(Parser)]
#[command(
    name = "mine-de-rien",
    version,
    about = "Point d'entrée en ligne de commande."
)]
struct Arguments {
    #[arg(long, default_value = "config.toml", global = true)]
    config: PathBuf,
    #[command(subcommand)]
    action: Option<Action>,
}

#[derive(Subcommand)]
enum Action {
    /// affiche un message à un joueur dans le chat
    Dire { joueur: String, message: String },
    /// affiche le chat des joueurs au fil de l'eau, pour vérifier
    Ecouter,
    /// construit ou répare l'atelier des coffres
    Atelier,
    /// met un exercice dans la file des coffres
    Coffre {
        /// un calcul (« 348 ÷ 12 ») ou une compétence (« division 2 »)
        texte: String,
    },
    /// surveille l'atelier et pose les coffres de la file
    Lancer,
    /// pose une question au LLM configuré, pour l'essayer
    Llm {
        consigne: String,
        /// photo JPEG jointe à la question
        #[arg(long)]
        image: Option<PathBuf>,
    },
    /// lit une fiche photographiée, sans rien mettre en file
    LirePhoto {
        image: PathBuf,
        /// partie de la page, ex. « exercice 7 »
        #[arg(long, default_value = "")]
        consigne: String,
        /// exercices à tirer au hasard
        #[arg(long, default_value_t = 3)]
        nombre: usize,
    },
    /// recopie le datapack dans un dossier lu par le serveur, sans config
    Datapack {
        /// reçoit le sous-dossier mine_de_rien
        dossier: PathBuf,
        /// datapack à recopier, à la place de celui du binaire
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

fn erreur(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("Erreur : {message}");
    ExitCode::FAILURE
}

pub fn main() -> ExitCode {
    let arguments = Arguments::parse();

    // Avant charger_config : le service qui livre le datapack n'a pas de config.toml.
    if let Some(Action::Datapack { dossier, source }) = &arguments.action {
        return match datapack::recopier(dossier, source.as_deref()) {
            Ok(cible) => {
                println!("Datapack recopié dans {}", cible.display());
                ExitCode::SUCCESS
            }
            Err(e) => erreur(e),
        };
    }

    let config = match charger_config(&arguments.config) {
        Ok(config) => config,
        Err(e) => return erreur(e),
    };
    executer(arguments.action, &config)
}

fn jeu(config: &Config) -> JavaRcon {
    let j = &config.jeu;
    JavaRcon::new(&j.hote, j.port, &j.mot_de_passe, &j.log, DELAI)
}

async fn dire(config: &Config, joueur: &str, message: &str) -> Result<(), ErreurRcon> {
    let jeu = jeu(config);
    let resultat = jeu.dire(joueur, message, &[]).await;
    jeu.fermer().await;
    resultat
}

async fn construire_atelier(config: &Config) -> Result<(), ErreurAtelier> {
    let Some(a) = config.atelier else {
        return Err(ErreurAtelier::Refus(
            "Section [atelier] absente de config.toml. Sans coordonnées : /atelier sur Telegram."
                .to_owned(),
        ));
    };
    let jeu = jeu(config);
    let resultat = atelier::construire(&jeu, &atelier::Atelier::new(a.x, a.y, a.z)).await;
    jeu.fermer().await;
    println!("{}", resultat?);
    Ok(())
}

fn depot(config: &Config) -> Result<Depot, ErreurStockage> {
    let chemin = std::path::Path::new(&config.stockage.sqlite);
    if let Some(dossier) = chemin.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dossier).map_err(|e| {
            ErreurStockage::Refus(format!(
                "dossier {} impossible à créer : {e}",
                dossier.display()
            ))
        })?;
    }
    Depot::ouvrir(chemin)
}

fn coffre(config: &Config, texte: &str) -> ExitCode {
    let depot = match depot(config) {
        Ok(depot) => depot,
        Err(e) => return erreur(e),
    };
    match mettre_en_file(&depot, &config.coffre, texte, "cli", &mut Alea::nouveau()) {
        Ok(lignes) => {
            println!("{}", lignes.join("\n"));
            ExitCode::SUCCESS
        }
        Err(ErreurFile::Saisie(e)) => {
            eprintln!("Refusé : {e}");
            ExitCode::FAILURE
        }
        Err(e) => erreur(e),
    }
}

/// Attend Ctrl+C ou l'arrêt demandé par Docker. Seul processus de son conteneur, l'agent ne
/// recevrait sinon aucun arrêt propre.
async fn arret_demande() {
    let terme = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = terme => {}
    }
}

async fn lancer(config: &Config) -> Result<(), String> {
    let pseudo = config.enfant.pseudo_minecraft.as_str();
    let jeu = Rc::new(jeu(config));
    let depot = Rc::new(depot(config).map_err(|e| e.to_string())?);
    let pixel_actif = config.assistant.actif && !config.jeu.log.is_empty();
    let assistant = pixel_actif.then_some(config.assistant.nom.as_str());
    let coffre = Rc::new(
        CoffreDuSoir::new(
            jeu.clone(),
            pseudo,
            config.coffre.niveaux_offerts,
            assistant,
        )
        .map_err(|e| e.to_string())?,
    );
    println!(
        "Surveillance de l'atelier, coffres dans {}. Ctrl+C pour arrêter.",
        config.stockage.sqlite
    );
    let parents = &config.canal.parents;
    let canal = if config.canal.jeton.is_empty() {
        println!("Pas de jeton Telegram : les notifications s'affichent ici.");
        None
    } else {
        if parents.is_empty() {
            eprintln!("Aucun parent dans [canal] parents : seul /start répondra.");
        }
        let canal = Rc::new(Telegram::new(&config.canal.jeton, parents));
        canal.demarrer().await.map_err(|e| e.to_string())?;
        println!("Telegram : {} parent(s) autorisé(s).", parents.len());
        Some(canal)
    };
    let llm = match creer_llm(&config.llm) {
        Ok(llm) => Some(llm),
        Err(e) => {
            eprintln!("LLM désactivé (photos, Pixel en mots-clés) : {e}");
            None
        }
    };
    let notificateur: Rc<dyn Notificateur> = match &canal {
        Some(canal) => Rc::new(NotificateurCanal::new(canal.clone())),
        None => Rc::new(NotificateurConsole),
    };
    let paisible = if config.paisible.actif {
        Some(
            ModePaisible::new(jeu.clone(), pseudo, &config.paisible.difficulte_normale)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let orchestrateur = Orchestrateur::new(
        depot.clone(),
        coffre.clone(),
        notificateur,
        Box::new(maintenant),
        paisible,
    );
    let pixel = if pixel_actif {
        let pixel = Pixel::new(
            jeu.clone(),
            depot.clone(),
            pseudo,
            &config.assistant.nom,
            llm.clone(),
            Box::new(maintenant),
        )
        .map_err(|e| e.to_string())?;
        let mode = if pixel.avec_llm() {
            "LLM local"
        } else {
            "mots-clés"
        };
        println!("{} écoute le chat ({mode}).", config.assistant.nom);
        Some(pixel)
    } else {
        None
    };
    let commandes = CommandesParents::new(
        depot,
        config.coffre.clone(),
        Some(coffre),
        parents,
        Box::new(maintenant),
        llm,
        Alea::nouveau(),
    );
    let ecoute = async {
        match &pixel {
            Some(pixel) => pixel.ecouter().await,
            None => std::future::pending().await,
        }
    };
    let service = async {
        match &canal {
            Some(canal) => servir(canal.as_ref(), &commandes).await,
            None => std::future::pending().await,
        }
    };
    let resultat = tokio::select! {
        () = orchestrateur.tourner(INTERVALLE) => Ok(()),
        fin = ecoute => fin.map_err(|e| e.to_string()),
        () = service => Ok(()),
        () = arret_demande() => Ok(()),
    };
    // La base se ferme avec le dépôt, à la sortie de la fonction.
    jeu.fermer().await;
    resultat
}

async fn interroger_llm(
    config: &Config,
    consigne: &str,
    image: Option<&std::path::Path>,
) -> Result<(), String> {
    let llm = creer_llm(&config.llm).map_err(|e| e.to_string())?;
    let octets = image
        .map(std::fs::read)
        .transpose()
        .map_err(|e| e.to_string())?;
    let reponse = llm
        .completer(
            "Tu réponds en français.",
            consigne,
            octets.as_deref(),
            false,
        )
        .await
        .map_err(|e| e.to_string())?;
    println!("{reponse}");
    Ok(())
}

async fn lire_photo(
    config: &Config,
    image: &std::path::Path,
    consigne: &str,
    nombre: usize,
) -> Result<(), String> {
    let llm = creer_llm(&config.llm).map_err(|e| e.to_string())?;
    let octets = std::fs::read(image).map_err(|e| e.to_string())?;
    let debut = std::time::Instant::now();
    let mut alea = Alea::nouveau();
    let lecture = photo::lire(llm.as_ref(), &octets, consigne, &mut alea)
        .await
        .map_err(|e| e.to_string())?;
    let mut lignes = photo::resumer(&lecture);
    lignes.push(format!(
        "Lu en {:.0} s. Tirage de {nombre} :",
        debut.elapsed().as_secs_f64()
    ));
    for ex in photo::tirer(&lecture, nombre, &mut alea) {
        lignes.push(format!("• {} → {}", ex.enonce, ex.reponse));
    }
    println!("{}", lignes.join("\n"));
    Ok(())
}

async fn ecouter(config: &Config) -> Result<(), ErreurRcon> {
    let mut flux = jeu(config).chat()?;
    println!("Chat lu dans {}. Ctrl+C pour arrêter.", config.jeu.log);
    loop {
        tokio::select! {
            message = flux.suivant() => println!(
                "{} <{}> {}",
                message.horodatage.with_timezone(&Local).format("%H:%M:%S"),
                message.joueur,
                message.texte
            ),
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

/// Lance une action asynchrone sur un runtime mono-fil : c'est le modèle d'asyncio, et rien
/// dans l'agent ne demande plusieurs fils.
fn attendre<T>(action: impl Future<Output = T>) -> std::io::Result<T> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    Ok(tokio::task::LocalSet::new().block_on(&runtime, action))
}

fn terminer<E: std::fmt::Display>(resultat: std::io::Result<Result<(), E>>) -> ExitCode {
    match resultat {
        Ok(Ok(())) => ExitCode::SUCCESS,
        Ok(Err(e)) => erreur(e),
        Err(e) => erreur(e),
    }
}

fn executer(action: Option<Action>, config: &Config) -> ExitCode {
    match action {
        None => {
            println!(
                "Mine de rien {VERSION} : jeu={} canal={} llm={}",
                config.jeu.adaptateur.nom(),
                config.canal.adaptateur.nom(),
                config.llm.adaptateur
            );
            ExitCode::SUCCESS
        }
        Some(Action::Dire { joueur, message }) => {
            terminer(attendre(dire(config, &joueur, &message)))
        }
        Some(Action::Ecouter) => terminer(attendre(ecouter(config))),
        Some(Action::Atelier) => terminer(attendre(construire_atelier(config))),
        Some(Action::Coffre { texte }) => coffre(config, &texte),
        Some(Action::Lancer) => terminer(attendre(lancer(config))),
        Some(Action::Llm { consigne, image }) => terminer(attendre(interroger_llm(
            config,
            &consigne,
            image.as_deref(),
        ))),
        Some(Action::LirePhoto {
            image,
            consigne,
            nombre,
        }) => terminer(attendre(lire_photo(config, &image, &consigne, nombre))),
        Some(Action::Datapack { .. }) => erreur("le datapack se recopie sans configuration"),
    }
}
