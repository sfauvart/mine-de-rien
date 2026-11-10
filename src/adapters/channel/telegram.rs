//! Canal Telegram, premier canal implémenté (`teloxide`, interrogation longue, sans son répartiteur).
//!
//! Seuls les identifiants listés dans `canal.parents` sont écoutés, et seulement en
//! conversation privée avec le bot : un message de groupe ou d'un inconnu est ignoré sans
//! réponse. Seule exception, `/start` répond à n'importe qui son propre identifiant Telegram,
//! pour que les parents puissent le recopier dans `config.toml`, et rien d'autre.
//!
//! Ce qui part vers Telegram est décidé ailleurs (orchestrateur, rapport) : énoncés, réponses,
//! numéros de coffre et durées, jamais le pseudo de l'enfant.
//!
//! Une photo de fiche envoyée par un parent est téléchargée en mémoire (la plus grande taille
//! proposée), jamais écrite sur le disque, et transmise avec sa légende. Elle a déjà traversé
//! les serveurs de Telegram : c'est le parent qui choisit ce qu'il photographie. Alternatives prévues sans compte
//! externe : Matrix auto-hébergé, page web sur le réseau local.
//!
//! Les mises à jour sont demandées à Telegram quand l'agent attend le message suivant
//! (`recevoir`) : pendant une lecture de photo, qui prend jusqu'à deux minutes, Telegram les
//! garde, et elles arrivent ensuite dans l'ordre.

use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use async_trait::async_trait;
use teloxide::Bot;
use teloxide::net::Download;
use teloxide::payloads::GetUpdatesSetters;
use teloxide::requests::Requester;
use teloxide::types::{ChatId, FileId, InputFile, Message, Update, UpdateKind};

use crate::adapters::channel::base::{Canal, MessageParent};
use crate::journal;
use crate::util::{debut, longueur};

/// Telegram refuse au-delà de 4096 caractères.
pub const TAILLE_MESSAGE: usize = 4000;
/// Durée de l'interrogation longue, sous le délai du client HTTP de `teloxide`.
const ATTENTE_TELEGRAM: u32 = 10;
const PAUSE_APRES_ERREUR: Duration = Duration::from_secs(5);

/// Le jeton est refusé, ou Telegram est injoignable au lancement.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurCanal(pub String);

/// Ce qu'il faut faire d'une mise à jour reçue de Telegram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tri {
    /// Groupe, inconnu, ou rien d'utilisable : aucune réponse.
    Ignorer,
    /// `/start` en conversation privée : on répond à n'importe qui son identifiant.
    Identifiant { conversation: i64, utilisateur: u64 },
    /// Un message texte d'un parent autorisé.
    Message(MessageParent),
    /// Une photo d'un parent autorisé, à télécharger : sa plus grande taille.
    Photo {
        expediteur: i64,
        legende: String,
        fichier: FileId,
    },
}

fn message_de(update: &Update) -> Option<&Message> {
    match &update.kind {
        UpdateKind::Message(message) | UpdateKind::EditedMessage(message) => Some(message),
        _ => None,
    }
}

/// `/start`, `/start@MonBot`, `/START quelque chose` : la commande, quelle que soit sa suite.
fn est_start(texte: &str) -> bool {
    let commande = texte.split_whitespace().next().unwrap_or_default();
    commande
        .split('@')
        .next()
        .unwrap_or_default()
        .eq_ignore_ascii_case("/start")
}

/// L'identifiant de l'expéditeur, s'il est parent et en conversation privée ; sinon `None`.
fn expediteur_autorise(message: &Message, parents: &HashSet<i64>) -> Option<i64> {
    let utilisateur = i64::try_from(message.from.as_ref()?.id.0).ok()?;
    (message.chat.is_private() && parents.contains(&utilisateur)).then_some(utilisateur)
}

pub fn trier(update: &Update, parents: &HashSet<i64>) -> Tri {
    let Some(message) = message_de(update) else {
        return Tri::Ignorer;
    };
    let texte = message.text().filter(|texte| !texte.is_empty());
    if let Some(texte) = texte {
        if est_start(texte) {
            return match &message.from {
                Some(utilisateur) if message.chat.is_private() => Tri::Identifiant {
                    conversation: message.chat.id.0,
                    utilisateur: utilisateur.id.0,
                },
                _ => Tri::Ignorer,
            };
        }
        return match expediteur_autorise(message, parents) {
            Some(expediteur) => Tri::Message(MessageParent::texte(expediteur, texte)),
            None => Tri::Ignorer,
        };
    }
    let Some(plus_grande) = message.photo().and_then(<[_]>::last) else {
        return Tri::Ignorer;
    };
    match expediteur_autorise(message, parents) {
        Some(expediteur) => Tri::Photo {
            expediteur,
            legende: message.caption().unwrap_or_default().to_owned(),
            fichier: plus_grande.file.id.clone(),
        },
        None => Tri::Ignorer,
    }
}

/// Coupe un long message sur les fins de ligne, en morceaux acceptés par Telegram.
/// Les longueurs se comptent en caractères, jamais en octets.
pub fn decouper(texte: &str, taille: usize) -> Vec<String> {
    let mut morceaux: Vec<String> = Vec::new();
    let mut courant = String::new();
    for ligne in texte.split('\n') {
        let mut ligne = ligne;
        while longueur(ligne) > taille {
            if !courant.is_empty() {
                morceaux.push(std::mem::take(&mut courant));
            }
            let tete = debut(ligne, taille);
            morceaux.push(tete.to_owned());
            ligne = &ligne[tete.len()..];
        }
        if courant.is_empty() {
            courant = ligne.to_owned();
        } else if longueur(&courant) + 1 + longueur(ligne) > taille {
            morceaux.push(std::mem::replace(&mut courant, ligne.to_owned()));
        } else {
            courant.push('\n');
            courant.push_str(ligne);
        }
    }
    if !courant.is_empty() {
        morceaux.push(courant);
    }
    morceaux
}

pub struct Telegram {
    bot: Bot,
    parents: HashSet<i64>,
    /// Numéro de la prochaine mise à jour à demander.
    suivante: Cell<i32>,
    attente: RefCell<VecDeque<MessageParent>>,
}

impl Telegram {
    pub fn new(jeton: &str, parents: &[i64]) -> Self {
        Self::avec_bot(Bot::new(jeton), parents)
    }

    /// Avec un client `teloxide` déjà réglé : les tests visent un faux serveur local.
    pub fn avec_bot(bot: Bot, parents: &[i64]) -> Self {
        Self {
            bot,
            parents: parents.iter().copied().collect(),
            suivante: Cell::new(0),
            attente: RefCell::new(VecDeque::new()),
        }
    }

    /// Vérifie le jeton auprès de Telegram avant de commencer.
    pub async fn demarrer(&self) -> Result<(), ErreurCanal> {
        match self.bot.get_me().await {
            Ok(_) => Ok(()),
            Err(e) => Err(ErreurCanal(format!(
                "Telegram a refusé le jeton ou est injoignable : {e}"
            ))),
        }
    }

    async fn telecharger(&self, fichier: FileId) -> Result<Vec<u8>, String> {
        let fichier = self
            .bot
            .get_file(fichier)
            .await
            .map_err(|e| e.to_string())?;
        let mut octets: Vec<u8> = Vec::new();
        self.bot
            .download_file(&fichier.path, &mut octets)
            .await
            .map_err(|e| e.to_string())?;
        Ok(octets)
    }

    /// Traite une mise à jour : répond à `/start`, ou met le message d'un parent en attente.
    pub async fn traiter(&self, update: &Update) {
        match trier(update, &self.parents) {
            Tri::Ignorer => {}
            Tri::Identifiant {
                conversation,
                utilisateur,
            } => {
                let reponse = format!("Ton identifiant Telegram : {utilisateur}");
                if let Err(e) = self.bot.send_message(ChatId(conversation), reponse).await {
                    journal::avertissement(format!("message Telegram non remis : {e}"));
                }
            }
            Tri::Message(message) => self.attente.borrow_mut().push_back(message),
            Tri::Photo {
                expediteur,
                legende,
                fichier,
            } => match self.telecharger(fichier).await {
                Ok(octets) => self.attente.borrow_mut().push_back(MessageParent {
                    expediteur,
                    texte: legende,
                    image: Some(octets),
                }),
                Err(e) => {
                    journal::avertissement(format!("photo Telegram non téléchargée : {e}"));
                    self.repondre(expediteur, "Je n'ai pas pu récupérer la photo, réessayer.")
                        .await;
                }
            },
        }
    }

    /// Demande les mises à jour suivantes à Telegram et les traite. Rend la main au plus tard
    /// après le délai de l'interrogation longue, même s'il n'y a rien.
    pub async fn interroger(&self) {
        let demande = self
            .bot
            .get_updates()
            .offset(self.suivante.get())
            .timeout(ATTENTE_TELEGRAM);
        match demande.await {
            Ok(updates) => {
                for update in updates {
                    self.suivante
                        .set(i32::try_from(update.id.0).map_or(0, |id| id + 1));
                    self.traiter(&update).await;
                }
            }
            Err(e) => {
                // Réseau coupé : on le signale, on attend un peu, on réessaie.
                journal::avertissement(format!("Telegram injoignable : {e}"));
                tokio::time::sleep(PAUSE_APRES_ERREUR).await;
            }
        }
    }
}

#[async_trait(?Send)]
impl Canal for Telegram {
    async fn repondre(&self, destinataire: i64, texte: &str) {
        if !self.parents.contains(&destinataire) {
            return;
        }
        for morceau in decouper(texte, TAILLE_MESSAGE) {
            // Réseau coupé, bot bloqué : on le signale, on continue.
            if let Err(e) = self.bot.send_message(ChatId(destinataire), morceau).await {
                journal::avertissement(format!("message Telegram non remis : {e}"));
                return;
            }
        }
    }

    async fn envoyer_audio(&self, destinataire: i64, nom: &str, ogg: &[u8]) {
        if !self.parents.contains(&destinataire) {
            return;
        }
        // En document : Telegram ne joue en message vocal que l'Ogg Opus, pas le Vorbis.
        let fichier = InputFile::memory(ogg.to_vec()).file_name(nom.to_owned());
        if let Err(e) = self.bot.send_document(ChatId(destinataire), fichier).await {
            journal::avertissement(format!("fichier audio Telegram non remis : {e}"));
        }
    }

    async fn envoyer(&self, texte: &str) {
        let mut parents: Vec<i64> = self.parents.iter().copied().collect();
        parents.sort_unstable();
        for parent in parents {
            self.repondre(parent, texte).await;
        }
    }

    async fn recevoir(&self) -> Option<MessageParent> {
        loop {
            if let Some(message) = self.attente.borrow_mut().pop_front() {
                return Some(message);
            }
            self.interroger().await;
        }
    }
}
