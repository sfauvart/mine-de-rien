//! Dire un son à l'enfant tout de suite, par le chat vocal du jeu.
//!
//! Le son ne voyage pas par le réseau de l'agent : il est déposé dans un dossier partagé avec
//! le serveur Minecraft, puis une commande RCON le désigne au plugin `mine-de-rien-voix`
//! (`plugin/`), qui le joue au seul joueur nommé. Le plugin lit le fichier en mémoire avant de
//! répondre : l'agent l'efface aussitôt, rien ne reste sur le disque.
//!
//! Une phrase de dictée, elle, est **gardée** dans le dossier tant que son coffre attend : le
//! plugin la relit à chaque clic droit de l'Écouteur, sans passer par l'agent.
//!
//! Les fichiers portent des numéros, jamais de mots : leur nom ne dit rien de ce qui est lu.
//!
//! **Les messages du chat** passent par un seul point, `Parole` : ce que l'agent écrit à
//! l'enfant (annonce, rappel, réponse de Pixel) est aussi lu par la voix locale et mis dans
//! la file. La voix s'ajoute au texte, elle ne le remplace pas et ne le retarde pas : les
//! lignes partent d'abord dans le chat, la voix suit. Elle ne dit rien que le chat n'écrit
//! pas, et se tait plutôt que de prononcer la réponse d'un coffre. Une voix en panne ou un
//! jeu sans chat vocal ne produisent aucune erreur : le message reste écrit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::rcon::ErreurRcon;
use crate::adapters::voix::base::Voix;
use crate::exercises::base::Exercice;
use crate::exercises::indices::contient_la_reponse;
use crate::journal;
use crate::minecraft::texte::{PseudoInvalide, verifier_pseudo};
use crate::son::{FREQUENCE_DU_CHAT_VOCAL, Pcm, lire_wav};
use crate::util::{repr, strip};

/// Le son n'a pas été dit ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurDiffusion(pub String);

impl From<ErreurRcon> for ErreurDiffusion {
    fn from(e: ErreurRcon) -> Self {
        Self(e.to_string())
    }
}

impl From<PseudoInvalide> for ErreurDiffusion {
    fn from(e: PseudoInvalide) -> Self {
        Self(e.to_string())
    }
}

/// Ce qui fait entendre un son à l'enfant. Un faux le remplace dans les tests.
#[async_trait(?Send)]
pub trait Diffusion {
    /// Met le son dans la file de l'enfant : il s'entend dès que le précédent est fini.
    async fn dire(&self, son: &Pcm) -> Result<(), ErreurDiffusion>;
    /// Vide la file et arrête le son en cours.
    async fn couper(&self) -> Result<(), ErreurDiffusion>;
    /// Vrai si le jeu de l'enfant est relié au chat vocal : il entendrait un son.
    async fn relie(&self) -> Result<bool, ErreurDiffusion>;
    /// Garde un son dans le dossier partagé sous `nom`, jusqu'à `oublier`.
    async fn garder(&self, nom: &str, son: &Pcm) -> Result<(), ErreurDiffusion>;
    /// Vrai si un son est gardé sous ce nom.
    async fn garde(&self, nom: &str) -> bool;
    /// Met un son gardé dans la file de l'enfant.
    async fn redire(&self, nom: &str) -> Result<(), ErreurDiffusion>;
    /// Désigne le son gardé que l'Écouteur rejoue au clic droit ; `None` : il redevient muet.
    async fn designer(&self, nom: Option<&str>) -> Result<(), ErreurDiffusion>;
    /// Efface un son gardé ; sans effet s'il n'y est pas.
    async fn oublier(&self, nom: &str) -> Result<(), ErreurDiffusion>;
}

/// Ce que le plugin répond à `mdrvoix etat`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    /// Le jeu de l'enfant est relié au chat vocal.
    Relie,
    /// L'enfant est en jeu, sans le mod ou avec le chat vocal coupé.
    SansChatVocal,
    /// L'enfant n'est pas en jeu.
    Absent,
    /// Le serveur a répondu autre chose : le plugin n'y est sans doute pas.
    Inconnu(String),
}

const LONGUEUR_MAX_DU_NOM: usize = 64;
const EXTENSION: &str = ".pcm";

/// Renvoie le nom s'il est sûr à placer dans une commande et à ouvrir dans le dossier
/// partagé : un nom seul, `[a-z0-9-]+\.pcm`. Le plugin refait ce contrôle de son côté.
pub fn verifier_fichier(nom: &str) -> Result<&str, ErreurDiffusion> {
    let permis = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
    match nom.strip_suffix(EXTENSION) {
        Some(racine)
            if !racine.is_empty()
                && racine.chars().count() <= LONGUEUR_MAX_DU_NOM
                && racine.chars().all(permis) =>
        {
            Ok(nom)
        }
        _ => Err(ErreurDiffusion(format!(
            "nom de fichier de son refusé : {}",
            repr(nom)
        ))),
    }
}

pub fn commande_jouer(pseudo: &str, fichier: &str) -> Result<String, ErreurDiffusion> {
    Ok(format!(
        "mdrvoix jouer {} {}",
        verifier_pseudo(pseudo)?,
        verifier_fichier(fichier)?
    ))
}

pub fn commande_couper(pseudo: &str) -> Result<String, ErreurDiffusion> {
    Ok(format!("mdrvoix couper {}", verifier_pseudo(pseudo)?))
}

/// Désigne la phrase de l'Écouteur, ou l'oublie si `fichier` est `None`.
pub fn commande_phrase(pseudo: &str, fichier: Option<&str>) -> Result<String, ErreurDiffusion> {
    let pseudo = verifier_pseudo(pseudo)?;
    Ok(match fichier {
        Some(fichier) => format!("mdrvoix phrase {pseudo} {}", verifier_fichier(fichier)?),
        None => format!("mdrvoix phrase {pseudo}"),
    })
}

pub fn commande_etat(pseudo: &str) -> Result<String, ErreurDiffusion> {
    Ok(format!("mdrvoix etat {}", verifier_pseudo(pseudo)?))
}

/// La diffusion par le chat vocal (Simple Voice Chat) et le plugin `mine-de-rien-voix`.
pub struct ChatVocal {
    jeu: Rc<dyn AdaptateurJeu>,
    pseudo: String,
    dossier: PathBuf,
    /// Deux sons déposés dans la même milliseconde ne portent pas le même nom.
    compteur: Cell<u32>,
}

impl ChatVocal {
    /// `dossier` est le dossier partagé, vu par l'agent (`[voix] dossier_du_jeu`).
    pub fn new(
        jeu: Rc<dyn AdaptateurJeu>,
        pseudo: &str,
        dossier: &Path,
    ) -> Result<Self, ErreurDiffusion> {
        verifier_pseudo(pseudo)?;
        if dossier.as_os_str().is_empty() {
            return Err(ErreurDiffusion(
                "[voix] dossier_du_jeu manquant : le dossier partagé avec le serveur Minecraft."
                    .to_owned(),
            ));
        }
        Ok(Self {
            jeu,
            pseudo: pseudo.to_owned(),
            dossier: dossier.to_owned(),
            compteur: Cell::new(0),
        })
    }

    fn nom_suivant(&self) -> String {
        let millisecondes = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        let numero = self.compteur.get();
        self.compteur.set(numero.wrapping_add(1));
        format!("son-{millisecondes}-{numero}{EXTENSION}")
    }

    /// Dépose le son, complet, sous `nom`. Le serveur tourne sous un autre compte : le
    /// fichier et le dossier lui sont lisibles, quel que soit le masque de l'agent.
    async fn deposer(&self, nom: &str, octets: &[u8]) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dossier).await?;
        tokio::fs::set_permissions(&self.dossier, std::fs::Permissions::from_mode(0o755)).await?;
        // Écrit sous un nom que le plugin refuse, puis renommé : il ne lit jamais un son
        // à moitié écrit.
        let provisoire = self.dossier.join(format!("{nom}.tmp"));
        tokio::fs::write(&provisoire, octets).await?;
        tokio::fs::set_permissions(&provisoire, std::fs::Permissions::from_mode(0o644)).await?;
        tokio::fs::rename(&provisoire, self.dossier.join(nom)).await
    }

    fn depot_impossible(&self, e: std::io::Error) -> ErreurDiffusion {
        ErreurDiffusion(format!(
            "son impossible à déposer dans {} : {e}",
            self.dossier.display()
        ))
    }

    /// Envoie une commande au plugin, qui répond « ok » ou dit pourquoi il refuse.
    async fn commander(&self, commande: &str) -> Result<(), ErreurDiffusion> {
        let reponse = self.jeu.commande(commande).await?;
        match strip(&reponse) {
            "ok" => Ok(()),
            "" => Err(ErreurDiffusion(
                "Le serveur n'a pas joué le son : aucune réponse".to_owned(),
            )),
            autre => Err(ErreurDiffusion(format!(
                "Le serveur n'a pas joué le son : {autre}"
            ))),
        }
    }

    /// Où en est le jeu de l'enfant, d'après le plugin.
    pub async fn etat(&self) -> Result<Etat, ErreurDiffusion> {
        let reponse = self.jeu.commande(&commande_etat(&self.pseudo)?).await?;
        Ok(match strip(&reponse) {
            "relié" => Etat::Relie,
            "sans chat vocal" => Etat::SansChatVocal,
            "absent" => Etat::Absent,
            autre => Etat::Inconnu(autre.to_owned()),
        })
    }
}

#[async_trait(?Send)]
impl Diffusion for ChatVocal {
    async fn dire(&self, son: &Pcm) -> Result<(), ErreurDiffusion> {
        if son.echantillons.is_empty() {
            return Err(ErreurDiffusion("Rien à dire : le son est vide.".to_owned()));
        }
        let nom = self.nom_suivant();
        let commande = commande_jouer(&self.pseudo, &nom)?;
        let octets = son.reechantillonner(FREQUENCE_DU_CHAT_VOCAL).en_pcm16();
        self.deposer(&nom, &octets)
            .await
            .map_err(|e| self.depot_impossible(e))?;
        let resultat = self.commander(&commande).await;
        // Joué ou refusé, le plugin n'a plus besoin du fichier.
        let _ = tokio::fs::remove_file(self.dossier.join(&nom)).await;
        resultat
    }

    async fn couper(&self) -> Result<(), ErreurDiffusion> {
        self.jeu.commande(&commande_couper(&self.pseudo)?).await?;
        Ok(())
    }

    async fn relie(&self) -> Result<bool, ErreurDiffusion> {
        Ok(self.etat().await? == Etat::Relie)
    }

    async fn garder(&self, nom: &str, son: &Pcm) -> Result<(), ErreurDiffusion> {
        verifier_fichier(nom)?;
        if son.echantillons.is_empty() {
            return Err(ErreurDiffusion(
                "Rien à garder : le son est vide.".to_owned(),
            ));
        }
        let octets = son.reechantillonner(FREQUENCE_DU_CHAT_VOCAL).en_pcm16();
        self.deposer(nom, &octets)
            .await
            .map_err(|e| self.depot_impossible(e))
    }

    async fn garde(&self, nom: &str) -> bool {
        verifier_fichier(nom).is_ok()
            && tokio::fs::metadata(self.dossier.join(nom))
                .await
                .is_ok_and(|m| m.is_file() && m.len() > 0)
    }

    async fn redire(&self, nom: &str) -> Result<(), ErreurDiffusion> {
        self.commander(&commande_jouer(&self.pseudo, nom)?).await
    }

    async fn designer(&self, nom: Option<&str>) -> Result<(), ErreurDiffusion> {
        self.commander(&commande_phrase(&self.pseudo, nom)?).await
    }

    async fn oublier(&self, nom: &str) -> Result<(), ErreurDiffusion> {
        verifier_fichier(nom)?;
        match tokio::fs::remove_file(self.dossier.join(nom)).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ErreurDiffusion(format!("son impossible à effacer : {e}"))),
        }
    }
}

/// Ce qui se lit à la place d'un trou « _____ ».
const TROU_PRONONCE: &str = "le mot manquant";

/// Les symboles d'un énoncé, tels qu'ils se disent. Les règles sont ici, pas dans un LLM.
const SYMBOLES: [(char, &str); 6] = [
    ('÷', "divisé par"),
    ('×', "fois"),
    ('+', "plus"),
    ('−', "moins"),
    ('=', "égale"),
    ('%', "pour cent"),
];

/// Le texte d'un message du chat tel que la voix le lit, ou `None` s'il n'y a rien à dire.
///
/// Les codes de couleur (« §a ») sont retirés ; un trou « _____ » se lit « le mot manquant » ;
/// « 348 ÷ 12 = ? » se lit « 348 divisé par 12 égale combien ? ». Un tiret entre deux nombres
/// est un « moins » ; ailleurs il reste un trait d'union.
pub fn prononcer(ecrit: &str) -> Option<String> {
    let caracteres: Vec<char> = ecrit.chars().collect();
    let mut mots: Vec<String> = Vec::new();
    let mut courant = String::new();
    let mut i = 0;
    let finir = |courant: &mut String, mots: &mut Vec<String>| {
        if !courant.is_empty() {
            mots.push(std::mem::take(courant));
        }
    };
    while i < caracteres.len() {
        let c = caracteres[i];
        if c == '§' {
            // Le code de couleur et son caractère.
            i += 2;
            continue;
        }
        if c == '_' {
            let debut = i;
            while i < caracteres.len() && caracteres[i] == '_' {
                i += 1;
            }
            finir(&mut courant, &mut mots);
            if i - debut >= 2 {
                mots.push(TROU_PRONONCE.to_owned());
            }
            continue;
        }
        if let Some((_, dit)) = SYMBOLES.iter().find(|(symbole, _)| *symbole == c) {
            finir(&mut courant, &mut mots);
            mots.push((*dit).to_owned());
        } else if c == '-'
            && courant.is_empty()
            && suit_un_nombre(&caracteres, i)
            && precede_d_un_nombre(&mots)
        {
            mots.push("moins".to_owned());
        } else if c.is_whitespace() {
            finir(&mut courant, &mut mots);
        } else {
            courant.push(c);
        }
        i += 1;
    }
    finir(&mut courant, &mut mots);
    // « égale ? » : la question se dit.
    let mut dits: Vec<String> = Vec::with_capacity(mots.len());
    for mot in mots {
        if mot == "?" && dits.last().is_some_and(|precedent| precedent == "égale") {
            dits.push("combien ?".to_owned());
        } else {
            dits.push(mot);
        }
    }
    let texte = dits.join(" ");
    texte.chars().any(char::is_alphanumeric).then_some(texte)
}

fn suit_un_nombre(caracteres: &[char], tiret: usize) -> bool {
    caracteres[tiret + 1..]
        .iter()
        .find(|c| !c.is_whitespace())
        .is_some_and(char::is_ascii_digit)
}

fn precede_d_un_nombre(mots: &[String]) -> bool {
    mots.last()
        .and_then(|mot| mot.chars().last())
        .is_some_and(|c| c.is_ascii_digit())
}

/// Le point de passage unique des messages du chat vers la voix.
///
/// `retenir` ne fait que noter le message, sans rien attendre : l'appelant écrit toutes ses
/// lignes dans le chat, puis appelle `dire`, qui les fait lire dans l'ordre.
pub struct Parole {
    diffusion: Rc<dyn Diffusion>,
    voix: Rc<dyn Voix>,
    attente: RefCell<VecDeque<String>>,
}

impl Parole {
    pub fn new(diffusion: Rc<dyn Diffusion>, voix: Rc<dyn Voix>) -> Self {
        Self {
            diffusion,
            voix,
            attente: RefCell::new(VecDeque::new()),
        }
    }

    /// Note un message écrit à l'enfant, pour le dire ensuite. `coffre` est l'exercice du
    /// coffre en cours, s'il y en a un : un texte qui en donnerait la réponse n'est pas dit,
    /// même si le chat l'écrit.
    pub fn retenir(&self, ecrit: &str, coffre: Option<&Exercice>) {
        let Some(texte) = prononcer(ecrit) else {
            return;
        };
        if coffre.is_some_and(|ex| contient_la_reponse(&texte, ex)) {
            return;
        }
        self.attente.borrow_mut().push_back(texte);
    }

    /// Fait lire par la voix ce qui a été retenu, dans l'ordre, et le met dans la file de
    /// l'enfant. Ne rend jamais d'erreur : sans chat vocal ou sans voix, les messages restent
    /// écrits, et rien d'autre ne se passe.
    pub async fn dire(&self) {
        let messages: Vec<String> = self.attente.borrow_mut().drain(..).collect();
        if messages.is_empty() {
            return;
        }
        // Avant de demander quoi que ce soit à la voix : l'enfant entendrait-il ?
        if !matches!(self.diffusion.relie().await, Ok(true)) {
            return;
        }
        for texte in messages {
            let dit = async {
                let wav = self
                    .voix
                    .dire(&texte, false)
                    .await
                    .map_err(|e| e.to_string())?;
                let son = lire_wav(&wav).map_err(|e| e.to_string())?;
                self.diffusion.dire(&son).await.map_err(|e| e.to_string())
            };
            if let Err(e) = dit.await {
                // Le texte du message n'est pas journalisé.
                journal::info(format!("message du chat non dit : {e}"));
                return;
            }
        }
    }
}
