//! Minecraft Java (Paper) via RCON pour les commandes et le log serveur pour le chat.
//!
//! Aucun plugin requis. Le chat est lu dans `logs/latest.log` (voir `log.rs` pour le format
//! observé et le suivi du fichier).

use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::log::{FluxChat, suivre_chat};
use crate::adapters::game::rcon::{ClientRcon, Echec, ErreurRcon};
use crate::minecraft::texte::{self, Style};
use crate::util::strip;

pub const DELAI: Duration = Duration::from_secs(5);

pub struct JavaRcon {
    pub hote: String,
    pub port: u16,
    pub log: String,
    // Une seule commande à la fois sur la connexion : les réponses ne se mélangent pas.
    client: Mutex<ClientRcon>,
}

impl JavaRcon {
    pub fn new(hote: &str, port: u16, mot_de_passe: &str, log: &str, delai: Duration) -> Self {
        Self {
            hote: hote.to_owned(),
            port,
            log: log.to_owned(),
            client: Mutex::new(ClientRcon::new(hote, port, mot_de_passe, delai)),
        }
    }

    pub async fn fermer(&self) {
        self.client.lock().await.fermer();
    }
}

#[async_trait(?Send)]
impl AdaptateurJeu for JavaRcon {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        let mut client = self.client.lock().await;
        // Reconnexion une fois, et seulement avant d'envoyer : une connexion qui tombe
        // pendant la réponse remonte en erreur, pour ne pas rejouer une commande déjà
        // exécutée (un `xp add` compté deux fois).
        if client.connexion_tombee().await {
            client.connecter().await?;
        }
        match client.commande(commande).await {
            Ok(reponse) => Ok(reponse),
            Err(Echec::Definitif(e)) => Err(e),
            Err(Echec::RienEnvoye(_)) => {
                client.connecter().await?;
                client
                    .commande(commande)
                    .await
                    .map_err(|echec| match echec {
                        Echec::Definitif(e) => e,
                        Echec::RienEnvoye(e) => {
                            ErreurRcon(format!("connexion RCON interrompue : {e}"))
                        }
                    })
            }
        }
    }

    async fn dire(
        &self,
        joueur: &str,
        message: &str,
        style: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon> {
        let reponse = self
            .commande(&texte::tellraw(joueur, message, style)?)
            .await?;
        // `tellraw` ne répond rien quand il réussit.
        let reponse = strip(&reponse);
        if !reponse.is_empty() {
            return Err(ErreurRcon(format!(
                "le serveur a refusé le message : {reponse}"
            )));
        }
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        if self.log.is_empty() {
            return Err(ErreurRcon(
                "[jeu] log manquant : le chemin de logs/latest.log du serveur.".to_owned(),
            ));
        }
        Ok(suivre_chat(&self.log))
    }
}
