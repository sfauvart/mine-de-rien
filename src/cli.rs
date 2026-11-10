//! Point d'entrée en ligne de commande.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use chrono::Local;

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::java_rcon::{DELAI, JavaRcon};
use crate::adapters::game::rcon::ErreurRcon;
use crate::config::{Config, charger_config};
use crate::games::atelier::{self, ErreurAtelier};
use crate::{VERSION, datapack};

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
        Some(_) => erreur("commande pas encore portée en Rust : utiliser la version Python."),
    }
}
