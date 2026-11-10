//! Point d'entrée en ligne de commande.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use crate::config::{Config, charger_config};
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
        Some(_) => erreur("commande pas encore portée en Rust : utiliser la version Python."),
    }
}
