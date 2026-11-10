//! Chat des joueurs lu dans `logs/latest.log`, sans plugin.
//!
//! Format observé (Paper, Minecraft 26.3) :
//!
//! ```text
//! [18:45:26] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> salut
//! ```
//!
//! `[Not Secure]` apparaît quand le chat n'est pas signé (comptes hors ligne) ; le nom du fil
//! varie. Les autres lignes sont ignorées : `/say` s'écrit `[Pseudo] texte`, et un joueur qui
//! tape « <autre> bonjour » produit `<lui> <autre> bonjour`, son propre pseudo en premier.
//!
//! Le fichier est suivi comme `tail -F` : à l'ouverture, on part de la fin (les messages d'avant
//! le lancement ne sont jamais rejoués). Le serveur renomme `latest.log` à chaque démarrage et au
//! changement de jour : si le fichier change (inode) ou rétrécit, il est relu depuis le début.
//!
//! Un message est daté de sa **réception** : l'heure du log est celle du conteneur, en UTC dans
//! l'image Docker (une heure d'écart observée le 2026-11-08), et elle ne porte pas la date.
//!
//! Le texte du chat n'est ni journalisé ni enregistré ici.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::Duration;

use chrono::{DateTime, Utc};
use regex::Regex;

use crate::adapters::game::base::MessageChat;
use crate::journal;

static LIGNE_CHAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^\[[0-9]{2}:[0-9]{2}:[0-9]{2}\] \[[^\]]+/INFO\]: (?:\[Not Secure\] )?<([A-Za-z0-9_]{3,16})> (.*)$",
    )
    .unwrap()
});
pub const INTERVALLE: Duration = Duration::from_millis(500);

/// Une ligne du log, lue à `quand` → message de chat, ou `None` si ce n'en est pas un.
pub fn analyser(ligne: &str, quand: DateTime<Utc>) -> Option<MessageChat> {
    let m = LIGNE_CHAT.captures(ligne.trim_end_matches(['\r', '\n']))?;
    Some(MessageChat {
        horodatage: quand,
        joueur: m[1].to_owned(),
        texte: m[2].to_owned(),
    })
}

/// Lit les lignes ajoutées à un fichier, rotation comprise. Une lecture par appel.
pub struct SuiviLog {
    chemin: PathBuf,
    fichier: Option<File>,
    inode: u64,
    position: u64,
    reste: Vec<u8>,
    // Au premier passage seulement, on saute ce qui est déjà écrit. Un fichier qui
    // apparaît ou qui remplace l'ancien est lu en entier : il est neuf.
    premier: bool,
}

impl SuiviLog {
    pub fn new(chemin: impl AsRef<Path>) -> Self {
        Self {
            chemin: chemin.as_ref().to_path_buf(),
            fichier: None,
            inode: 0,
            position: 0,
            reste: Vec::new(),
            premier: true,
        }
    }

    pub fn fermer(&mut self) {
        self.fichier = None;
        self.reste.clear();
    }

    pub fn lire(&mut self) -> io::Result<Vec<String>> {
        let etat = match std::fs::metadata(&self.chemin) {
            Ok(etat) => etat,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.fermer();
                self.premier = false;
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        if self.fichier.is_none() || etat.ino() != self.inode || etat.len() < self.position {
            self.fermer();
            self.fichier = Some(File::open(&self.chemin)?);
            self.inode = etat.ino();
            self.position = if self.premier { etat.len() } else { 0 };
        }
        self.premier = false;
        let Some(fichier) = self.fichier.as_mut() else {
            return Ok(Vec::new());
        };
        fichier.seek(SeekFrom::Start(self.position))?;
        let mut donnees = std::mem::take(&mut self.reste);
        let lus = fichier.read_to_end(&mut donnees)?;
        self.position += lus as u64;
        let mut morceaux: Vec<&[u8]> = donnees.split(|octet| *octet == b'\n').collect();
        let reste = morceaux.pop().unwrap_or_default().to_vec();
        let lignes = morceaux
            .iter()
            .map(|l| String::from_utf8_lossy(l).into_owned())
            .collect();
        self.reste = reste;
        Ok(lignes)
    }
}

/// Flux des messages de chat écrits dans le log à partir de sa création.
pub struct FluxChat {
    suivi: SuiviLog,
    intervalle: Duration,
    horloge: Box<dyn Fn() -> DateTime<Utc>>,
    attente: VecDeque<MessageChat>,
}

impl FluxChat {
    pub fn new(
        chemin: impl AsRef<Path>,
        intervalle: Duration,
        horloge: impl Fn() -> DateTime<Utc> + 'static,
    ) -> Self {
        Self {
            suivi: SuiviLog::new(chemin),
            intervalle,
            horloge: Box::new(horloge),
            attente: VecDeque::new(),
        }
    }

    /// Le prochain message de chat ; attend aussi longtemps qu'il le faut.
    pub async fn suivant(&mut self) -> MessageChat {
        loop {
            if let Some(message) = self.attente.pop_front() {
                return message;
            }
            // Lecture courte d'un fichier local : faite sur place, sans fil auxiliaire.
            match self.suivi.lire() {
                Ok(lignes) => {
                    let quand = (self.horloge)();
                    self.attente
                        .extend(lignes.iter().filter_map(|l| analyser(l, quand)));
                }
                Err(e) => journal::avertissement(format!("log du serveur illisible : {e}")),
            }
            if self.attente.is_empty() {
                tokio::time::sleep(self.intervalle).await;
            }
        }
    }
}

/// Flux des messages de chat écrits dans le log à partir de maintenant.
pub fn suivre_chat(chemin: impl AsRef<Path>) -> FluxChat {
    FluxChat::new(chemin, INTERVALLE, Utc::now)
}
