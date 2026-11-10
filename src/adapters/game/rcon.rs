//! Client RCON minimal, asynchrone.
//!
//! Protocole : paquets `<longueur:int32><id:int32><type:int32><corps>\0\0`, en petit-boutiste.
//!
//! Le serveur vanilla lit chaque paquet d'un seul `read` et ignore ce qui suit dans le même
//! tampon : deux paquets envoyés coup sur coup peuvent se coller, le second est perdu et la
//! connexion se ferme. On n'a donc jamais plus d'un paquet en attente de réponse. C'est pour
//! garder la main là-dessus que le client est écrit ici plutôt que pris dans un crate.
//!
//! Une réponse longue arrive découpée en morceaux de 4096 caractères. Si le premier morceau est
//! plein, on envoie alors seulement un paquet sentinelle d'un type inconnu : le serveur y répond
//! « Unknown request » après avoir fini d'écrire la réponse, ce qui en marque la fin.

use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::minecraft::texte::PseudoInvalide;

const CONNEXION: i32 = 3;
const COMMANDE: i32 = 2;
const SENTINELLE: i32 = 0;

/// Le serveur vanilla refuse les paquets entrants dont le corps dépasse cette taille, en octets.
pub const TAILLE_MAX_COMMANDE: usize = 1446;
/// Taille des morceaux d'une réponse longue, en caractères (donc au moins autant d'octets).
pub const TAILLE_MORCEAU: usize = 4096;

/// Le serveur est injoignable, a refusé le mot de passe ou a répondu de travers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurRcon(pub String);

impl From<PseudoInvalide> for ErreurRcon {
    fn from(e: PseudoInvalide) -> Self {
        Self(e.to_string())
    }
}

/// Pourquoi une commande n'a pas abouti. Si rien n'est parti, l'appelant peut se reconnecter
/// et réessayer ; sinon la commande a peut-être été exécutée, et on ne la rejoue pas.
#[derive(Debug)]
pub(crate) enum Echec {
    RienEnvoye(io::Error),
    Definitif(ErreurRcon),
}

pub struct ClientRcon {
    hote: String,
    port: u16,
    mot_de_passe: String,
    delai: Duration,
    flux: Option<TcpStream>,
    prochain_id: i32,
}

fn hors_delai() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "délai dépassé")
}

impl ClientRcon {
    pub fn new(hote: &str, port: u16, mot_de_passe: &str, delai: Duration) -> Self {
        Self {
            hote: hote.to_owned(),
            port,
            mot_de_passe: mot_de_passe.to_owned(),
            delai,
            flux: None,
            prochain_id: 0,
        }
    }

    pub fn connecte(&self) -> bool {
        self.flux.is_some()
    }

    pub async fn connecter(&mut self) -> Result<(), ErreurRcon> {
        self.fermer();
        let adresse = (self.hote.as_str(), self.port);
        let flux = match timeout(self.delai, TcpStream::connect(adresse)).await {
            Ok(Ok(flux)) => flux,
            Ok(Err(e)) => return Err(self.injoignable(&e)),
            Err(_) => return Err(self.injoignable(&hors_delai())),
        };
        self.flux = Some(flux);
        let mot_de_passe = self.mot_de_passe.clone();
        let echange = async {
            let ident = self.envoyer(CONNEXION, &mot_de_passe).await?;
            let (reponse_id, _) = self.lire_paquet().await?;
            Ok::<_, io::Error>((ident, reponse_id))
        };
        let (ident, reponse_id) = match echange.await {
            Ok(ids) => ids,
            Err(e) => {
                self.fermer();
                return Err(ErreurRcon(format!("connexion RCON interrompue : {e}")));
            }
        };
        if reponse_id == -1 {
            self.fermer();
            return Err(ErreurRcon("mot de passe RCON refusé".to_owned()));
        }
        if reponse_id != ident {
            self.fermer();
            return Err(ErreurRcon(
                "réponse RCON inattendue à l'authentification".to_owned(),
            ));
        }
        Ok(())
    }

    fn injoignable(&self, e: &io::Error) -> ErreurRcon {
        ErreurRcon(format!(
            "serveur RCON injoignable sur {}:{} : {e}",
            self.hote, self.port
        ))
    }

    pub fn fermer(&mut self) {
        self.flux = None;
    }

    /// Vrai si le serveur a fermé la connexion depuis le dernier échange.
    ///
    /// Le test se fait avant d'envoyer, pour pouvoir se reconnecter sans risquer
    /// d'exécuter deux fois une commande.
    pub async fn connexion_tombee(&mut self) -> bool {
        let Some(flux) = &self.flux else { return true };
        let mut octet = [0u8; 1];
        // Délai nul : on regarde ce qui est déjà arrivé, sans attendre.
        match timeout(Duration::ZERO, flux.peek(&mut octet)).await {
            Err(_) => false,
            Ok(Ok(lus)) => lus == 0,
            Ok(Err(_)) => true,
        }
    }

    /// Envoie une commande et renvoie la réponse complète du serveur.
    pub(crate) async fn commande(&mut self, commande: &str) -> Result<String, Echec> {
        if commande.len() > TAILLE_MAX_COMMANDE {
            return Err(Echec::Definitif(ErreurRcon(format!(
                "commande trop longue (plus de {TAILLE_MAX_COMMANDE} octets)"
            ))));
        }
        if self.flux.is_none() {
            return Err(Echec::Definitif(ErreurRcon(
                "pas de connexion RCON".to_owned(),
            )));
        }
        // Une erreur à cet envoi-ci remonte telle quelle : rien n'est parti, l'appelant peut
        // se reconnecter et réessayer.
        let ident = self
            .envoyer(COMMANDE, commande)
            .await
            .map_err(Echec::RienEnvoye)?;
        match self.lire_reponse(ident).await {
            Ok(octets) => Ok(String::from_utf8_lossy(&octets).into_owned()),
            Err(e) => {
                // La commande a peut-être été exécutée : on ne la rejoue pas.
                self.fermer();
                Err(Echec::Definitif(ErreurRcon(format!(
                    "connexion RCON interrompue pendant la réponse : {e}"
                ))))
            }
        }
    }

    async fn lire_reponse(&mut self, ident: i32) -> io::Result<Vec<u8>> {
        let (reponse_id, mut reponse) = self.lire_paquet().await?;
        if reponse_id != ident {
            return Err(io::Error::other(format!(
                "réponse RCON inattendue (id {reponse_id} au lieu de {ident})"
            )));
        }
        if reponse.len() >= TAILLE_MORCEAU {
            let sentinelle = self.envoyer(SENTINELLE, "").await?;
            loop {
                let (reponse_id, corps) = self.lire_paquet().await?;
                if reponse_id == sentinelle {
                    break;
                }
                if reponse_id == ident {
                    reponse.extend_from_slice(&corps);
                }
            }
        }
        Ok(reponse)
    }

    async fn envoyer(&mut self, genre: i32, corps: &str) -> io::Result<i32> {
        self.prochain_id = self.prochain_id % 2_000_000_000 + 1;
        let ident = self.prochain_id;
        let longueur = (corps.len() + 10) as i32;
        let mut paquet = Vec::with_capacity(corps.len() + 14);
        paquet.extend_from_slice(&longueur.to_le_bytes());
        paquet.extend_from_slice(&ident.to_le_bytes());
        paquet.extend_from_slice(&genre.to_le_bytes());
        paquet.extend_from_slice(corps.as_bytes());
        paquet.extend_from_slice(&[0, 0]);
        let delai = self.delai;
        let flux = self
            .flux
            .as_mut()
            .ok_or_else(|| io::Error::other("pas de connexion RCON"))?;
        timeout(delai, flux.write_all(&paquet))
            .await
            .map_err(|_| hors_delai())??;
        Ok(ident)
    }

    /// L'identifiant et le corps du prochain paquet.
    async fn lire_paquet(&mut self) -> io::Result<(i32, Vec<u8>)> {
        let mut entete = [0u8; 4];
        self.lire(&mut entete).await?;
        let longueur = i32::from_le_bytes(entete);
        if !(10..=1 << 20).contains(&longueur) {
            return Err(io::Error::other(format!(
                "longueur de paquet RCON invalide : {longueur}"
            )));
        }
        let mut charge = vec![0u8; longueur as usize];
        self.lire(&mut charge).await?;
        let ident = i32::from_le_bytes([charge[0], charge[1], charge[2], charge[3]]);
        Ok((ident, charge[8..charge.len() - 2].to_vec()))
    }

    async fn lire(&mut self, tampon: &mut [u8]) -> io::Result<()> {
        let delai = self.delai;
        let flux = self
            .flux
            .as_mut()
            .ok_or_else(|| io::Error::other("pas de connexion RCON"))?;
        match timeout(delai, flux.read_exact(tampon)).await {
            Err(_) => Err(hors_delai()),
            Ok(Err(e)) if e.kind() == io::ErrorKind::UnexpectedEof => Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "connexion fermée par le serveur",
            )),
            Ok(resultat) => resultat.map(|_| ()),
        }
    }
}
