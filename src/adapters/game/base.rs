//! Contrat d'un adaptateur de jeu.

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::adapters::game::log::FluxChat;
use crate::adapters::game::rcon::ErreurRcon;
use crate::minecraft::texte::Style;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageChat {
    pub horodatage: DateTime<Utc>,
    pub joueur: String,
    pub texte: String,
}

/// Runtime mono-fil : rien ici n'a besoin de traverser un fil, d'où `?Send`.
#[async_trait(?Send)]
pub trait AdaptateurJeu {
    /// Exécute une commande serveur (sans le / initial) et renvoie sa sortie brute.
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon>;

    /// Affiche un message à un joueur (style : `("color", Style::Texte("gold"))`, ...).
    async fn dire(
        &self,
        joueur: &str,
        texte: &str,
        style: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon>;

    /// Flux des messages de chat des joueurs.
    fn chat(&self) -> Result<FluxChat, ErreurRcon>;
}
