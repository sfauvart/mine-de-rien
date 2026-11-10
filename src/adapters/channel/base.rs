//! Contrat d'un canal de communication avec les parents.

use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageParent {
    pub expediteur: i64,
    pub texte: String,
    /// Une photo jointe, en mémoire : elle n'est jamais écrite sur le disque.
    pub image: Option<Vec<u8>>,
}

impl MessageParent {
    pub fn texte(expediteur: i64, texte: &str) -> Self {
        Self {
            expediteur,
            texte: texte.to_owned(),
            image: None,
        }
    }
}

#[async_trait(?Send)]
pub trait Canal {
    /// Envoie un message à tous les parents autorisés.
    async fn envoyer(&self, texte: &str);

    /// Répond à un parent autorisé.
    async fn repondre(&self, destinataire: i64, texte: &str);

    /// Envoie à un parent autorisé un fichier audio (Ogg Vorbis) à écouter : l'aperçu d'une
    /// dictée lue par la voix de synthèse. Un canal qui ne sait pas le faire ne fait rien.
    async fn envoyer_audio(&self, _destinataire: i64, _nom: &str, _ogg: &[u8]) {}

    /// Le prochain message reçu d'un parent autorisé ; `None` quand le canal est fermé.
    async fn recevoir(&self) -> Option<MessageParent>;
}
