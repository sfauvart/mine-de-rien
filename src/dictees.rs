//! De la dictée aux sons : ce qui relie le texte (`exercises/francais/dictee.rs`), la voix
//! (`adapters/voix`), la conversion (`son.rs`) et le chat vocal (`diffusion.rs`).
//!
//! Chaque phrase a deux sons, à vitesse normale et à vitesse lente. Leurs numéros se déduisent
//! du numéro de la phrase : `2 × phrase` et `2 × phrase + 1`. Un numéro ne dit rien du texte.
//! La voix relit la phrase quand son coffre arrive (`Lecteur`) : les sons que la base garde
//! depuis l'époque du pack de ressources ne sont plus joués.

use std::collections::HashMap;
use std::rc::Rc;

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc, Weekday};
use serde_json::{Value, json};

use crate::adapters::voix::base::{ErreurVoix, Voix};
use crate::alea::Alea;
use crate::diffusion::{Diffusion, ErreurDiffusion};
use crate::exercises::base::{Exercice, normaliser};
use crate::exercises::francais::dictee::{
    ErreurDictee, Phrase, Position, candidats, decouper, decouper_liste, exercice, phrase_de, tirer,
};
use crate::games::tableau::{Morceau, Trou, morceaux};
use crate::son::{ErreurSon, Pcm, encoder_ogg, lire_wav, wav_vers_ogg};
use crate::storage::sqlite::{
    Demande, Depot, DicteeStockee, ErreurStockage, ExerciceStocke, PhraseSonore, Statut,
};

#[derive(Debug, thiserror::Error)]
pub enum ErreurDictees {
    #[error("{0}")]
    Dictee(#[from] ErreurDictee),
    #[error("{0}")]
    Voix(#[from] ErreurVoix),
    #[error("{0}")]
    Son(#[from] ErreurSon),
    #[error("{0}")]
    Stockage(#[from] ErreurStockage),
    #[error("{0}")]
    Diffusion(#[from] ErreurDiffusion),
}

/// Au-delà, ce n'est plus une dictée de CM2, et la voix mettrait des minutes.
pub const PHRASES_MAX: usize = 40;
/// Une dictée reste révisable 30 jours après son jour : un mot raté peut revenir.
pub const JOURS_GARDES: i64 = 30;
/// Silence entre deux phrases de l'aperçu envoyé au parent.
const SILENCE_SECONDES: f64 = 0.8;

/// Le numéro du son de la phrase n°`phrase` (comptée à partir de 1).
pub fn numero_son(phrase: u32, lent: bool) -> u32 {
    phrase * 2 + u32::from(lent)
}

/// L'exercice d'un trou, avec les numéros des sons de sa phrase, que le jeu lira.
pub fn exercice_sonore(
    phrases: &[Phrase],
    position: Position,
    phrase: u32,
) -> Result<Exercice, ErreurDictee> {
    let mut ex = exercice(phrases, position)?;
    ex.donnees
        .insert("son".to_owned(), json!(numero_son(phrase, false)));
    ex.donnees
        .insert("son_lent".to_owned(), json!(numero_son(phrase, true)));
    Ok(ex)
}

/// Une dictée lue par la voix, en attente du « /oui » du parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Preparation {
    /// Le texte tel qu'il sera gardé : les phrases, une par ligne.
    pub texte: String,
    pub phrases: Vec<Phrase>,
    pub sonores: Vec<PhraseSonore>,
    /// Toutes les phrases à la suite, à vitesse normale, pour que le parent écoute.
    pub apercu: Vec<u8>,
}

impl Preparation {
    pub fn mots_a_demander(&self) -> usize {
        candidats(&self.phrases).len()
    }
}

/// Découpe le texte (ou la liste de mots) et le fait lire aux deux vitesses.
pub async fn preparer(
    voix: &dyn Voix,
    texte: &str,
    liste: bool,
) -> Result<Preparation, ErreurDictees> {
    let phrases = if liste {
        decouper_liste(texte)
    } else {
        decouper(texte)
    };
    if candidats(&phrases).is_empty() {
        return Err(ErreurDictee("Aucun mot à demander dans ce texte.".to_owned()).into());
    }
    if phrases.len() > PHRASES_MAX {
        return Err(ErreurDictee(format!(
            "{} phrases : c'est trop pour une dictée ({PHRASES_MAX} au plus).",
            phrases.len()
        ))
        .into());
    }
    let mut sonores = Vec::with_capacity(phrases.len());
    let mut suite: Option<Pcm> = None;
    for phrase in &phrases {
        let normal = lire_wav(&voix.dire(&phrase.texte, false).await?)?;
        let lent = wav_vers_ogg(&voix.dire(&phrase.texte, true).await?)?;
        sonores.push(PhraseSonore {
            texte: phrase.texte.clone(),
            son: encoder_ogg(&normal)?,
            son_lent: lent,
        });
        match &mut suite {
            None => suite = Some(normal),
            Some(suite) => {
                let silence = (SILENCE_SECONDES * f64::from(suite.frequence)) as usize;
                suite.echantillons.extend(std::iter::repeat_n(0.0, silence));
                // Une même voix garde sa fréquence d'une phrase à l'autre.
                suite.echantillons.extend(&normal.echantillons);
            }
        }
    }
    let apercu = match &suite {
        Some(suite) => encoder_ogg(suite)?,
        None => Vec::new(),
    };
    Ok(Preparation {
        texte: phrases
            .iter()
            .map(|p| p.texte.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        phrases,
        sonores,
        apercu,
    })
}

/// Enregistre une dictée confirmée. Rend le numéro de la dictée.
pub fn enregistrer(
    depot: &Depot,
    preparation: &Preparation,
    jour: NaiveDate,
    demande_par: &str,
    quand: DateTime<Utc>,
) -> Result<i64, ErreurDictees> {
    Ok(depot.ajouter_dictee(
        jour,
        &preparation.texte,
        &preparation.sonores,
        demande_par,
        quand,
    )?)
}

/// Les phrases d'une dictée enregistrée, avec le numéro de chacune dans la base.
pub fn phrases_enregistrees(
    depot: &Depot,
    dictee: i64,
) -> Result<(Vec<Phrase>, Vec<u32>), ErreurDictees> {
    let mut phrases = Vec::new();
    let mut numeros = Vec::new();
    for stockee in depot.phrases_de(dictee)? {
        let (Some(phrase), Ok(numero)) = (phrase_de(&stockee.texte), u32::try_from(stockee.id))
        else {
            return Err(ErreurDictee(format!(
                "La phrase n°{} de la dictée n°{dictee} est illisible.",
                stockee.id
            ))
            .into());
        };
        phrases.push(phrase);
        numeros.push(numero);
    }
    Ok((phrases, numeros))
}

/// Tire une séance de `n` trous dans une dictée enregistrée et met les coffres en file.
/// Rend, pour chacun, son numéro de coffre et l'exercice.
pub fn seance(
    depot: &Depot,
    dictee: &DicteeStockee,
    n: usize,
    alea: &mut Alea,
    poids: &HashMap<Position, f64>,
    demande: Demande<'_>,
    quand: DateTime<Utc>,
) -> Result<Vec<(i64, Exercice)>, ErreurDictees> {
    let (phrases, numeros) = phrases_enregistrees(depot, dictee.id)?;
    let mut coffres = Vec::new();
    for position in tirer(&phrases, n, alea, poids, &dictee.imposes)? {
        let mut ex = exercice_sonore(&phrases, position, numeros[position.phrase])?;
        ex.donnees.insert("dictee".to_owned(), json!(dictee.id));
        let numero = depot.ajouter(&ex, demande, quand)?;
        coffres.push((numero, ex));
    }
    Ok(coffres)
}

/// Le nom, dans le dossier partagé avec le serveur, du son n°`numero` : un numéro, jamais un
/// mot de la phrase.
pub fn fichier_du_son(numero: u32) -> String {
    format!("phrase-{numero}.pcm")
}

/// Ce qui fait entendre à l'enfant la phrase d'un coffre de dictée, par le chat vocal.
///
/// Les deux sons de la phrase (normal et lent) sont gardés dans le dossier partagé tant que le
/// coffre attend. S'ils manquent, la voix relit la phrase, retrouvée dans la base : rien ne
/// dépend d'un son fabriqué le jour où le parent a envoyé la dictée.
pub struct Lecteur {
    diffusion: Rc<dyn Diffusion>,
    voix: Rc<dyn Voix>,
    depot: Rc<Depot>,
}

impl Lecteur {
    pub fn new(diffusion: Rc<dyn Diffusion>, voix: Rc<dyn Voix>, depot: Rc<Depot>) -> Self {
        Self {
            diffusion,
            voix,
            depot,
        }
    }

    /// La diffusion sous-jacente, pour ce qui n'est pas une phrase de dictée.
    pub fn diffusion(&self) -> &Rc<dyn Diffusion> {
        &self.diffusion
    }

    /// Vrai si le jeu de l'enfant est relié au chat vocal.
    pub async fn relie(&self) -> Result<bool, ErreurDictees> {
        Ok(self.diffusion.relie().await?)
    }

    /// Les numéros des sons de la phrase d'un coffre : le normal, et le lent s'il existe.
    fn numeros(ex: &Exercice) -> Result<(u32, Option<u32>), ErreurDictees> {
        let numero = |cle: &str| {
            ex.donnees
                .get(cle)
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
        };
        match numero("son") {
            Some(normal) => Ok((normal, numero("son_lent"))),
            None => {
                Err(ErreurDictee("Ce coffre de dictée n'a pas de phrase à lire.".to_owned()).into())
            }
        }
    }

    /// Le texte de la phrase d'un coffre, relu dans la base.
    fn texte(&self, ex: &Exercice, normal: u32) -> Result<String, ErreurDictees> {
        let introuvable =
            || ErreurDictee("La phrase de ce coffre de dictée est introuvable.".to_owned());
        let dictee = ex
            .donnees
            .get("dictee")
            .and_then(Value::as_i64)
            .ok_or_else(introuvable)?;
        self.depot
            .phrases_de(dictee)?
            .into_iter()
            .find(|p| u32::try_from(p.id).is_ok_and(|id| numero_son(id, false) == normal))
            .map(|p| p.texte)
            .ok_or_else(|| introuvable().into())
    }

    /// S'assure que les deux sons de la phrase sont dans le dossier partagé ; la voix ne relit
    /// que ceux qui manquent.
    pub async fn preparer(&self, ex: &Exercice) -> Result<(), ErreurDictees> {
        let (normal, lent) = Self::numeros(ex)?;
        for (numero, lentement) in [(Some(normal), false), (lent, true)] {
            let Some(numero) = numero else {
                continue;
            };
            let nom = fichier_du_son(numero);
            if self.diffusion.garde(&nom).await {
                continue;
            }
            let texte = self.texte(ex, normal)?;
            let son = lire_wav(&self.voix.dire(&texte, lentement).await?)?;
            self.diffusion.garder(&nom, &son).await?;
        }
        Ok(())
    }

    /// Désigne la phrase du coffre à l'Écouteur : un clic droit la rejoue.
    pub async fn designer(&self, ex: &Exercice) -> Result<(), ErreurDictees> {
        self.preparer(ex).await?;
        let (normal, _) = Self::numeros(ex)?;
        Ok(self
            .diffusion
            .designer(Some(&fichier_du_son(normal)))
            .await?)
    }

    /// Fait entendre la phrase du coffre, à vitesse normale ou lente (à défaut de son lent,
    /// le normal).
    pub async fn dire(&self, ex: &Exercice, lent: bool) -> Result<(), ErreurDictees> {
        self.preparer(ex).await?;
        let (normal, lentement) = Self::numeros(ex)?;
        let numero = lentement.filter(|_| lent).unwrap_or(normal);
        Ok(self.diffusion.redire(&fichier_du_son(numero)).await?)
    }

    /// Le coffre est ouvert ou retiré : l'Écouteur redevient muet, les sons sont effacés.
    pub async fn oublier(&self, ex: &Exercice) -> Result<(), ErreurDictees> {
        let muet = self.diffusion.designer(None).await;
        if let Ok((normal, lent)) = Self::numeros(ex) {
            for numero in [Some(normal), lent].into_iter().flatten() {
                self.diffusion.oublier(&fichier_du_son(numero)).await?;
            }
        }
        Ok(muet?)
    }
}

const JOURS: [&str; 7] = [
    "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
];
const MOIS: [&str; 12] = [
    "janvier",
    "février",
    "mars",
    "avril",
    "mai",
    "juin",
    "juillet",
    "août",
    "septembre",
    "octobre",
    "novembre",
    "décembre",
];

/// « vendredi 13 novembre » : un jour tel qu'on le dit aux parents.
pub fn ecrire_jour(jour: NaiveDate) -> String {
    let nom = JOURS[jour.weekday().num_days_from_monday() as usize];
    let mois = MOIS[jour.month0() as usize];
    let quantieme = if jour.day() == 1 {
        "1er".to_owned()
    } else {
        jour.day().to_string()
    };
    format!("{nom} {quantieme} {mois}")
}

/// Le jour de la dictée, tel que le parent l'écrit : un nom de jour (le prochain, jamais
/// aujourd'hui) ou `JJ/MM`.
pub fn lire_jour(texte: &str, aujourdhui: NaiveDate) -> Result<NaiveDate, ErreurDictee> {
    let incompris = || {
        ErreurDictee(
            "Je ne comprends pas le jour de la dictée. Exemples : « vendredi » ou « 13/11 »."
                .to_owned(),
        )
    };
    let cle = normaliser(texte);
    if let Some(rang) = JOURS.iter().position(|j| normaliser(j) == cle) {
        let cible = Weekday::try_from(rang as u8).map_err(|_| incompris())?;
        let mut jour = aujourdhui + TimeDelta::days(1);
        while jour.weekday() != cible {
            jour += TimeDelta::days(1);
        }
        return Ok(jour);
    }
    let (quantieme, mois) = texte.trim().split_once('/').ok_or_else(incompris)?;
    let (Ok(quantieme), Ok(mois)) = (quantieme.parse::<u32>(), mois.parse::<u32>()) else {
        return Err(incompris());
    };
    let cette_annee =
        NaiveDate::from_ymd_opt(aujourdhui.year(), mois, quantieme).ok_or_else(incompris)?;
    if cette_annee > aujourdhui {
        return Ok(cette_annee);
    }
    // En décembre, « 08/01 » est en janvier prochain ; « 13/11 » dit le 14 est une erreur.
    match NaiveDate::from_ymd_opt(aujourdhui.year() + 1, mois, quantieme) {
        Some(prochaine) if (aujourdhui - cette_annee).num_days() > 180 => Ok(prochaine),
        _ => Err(ErreurDictee(format!(
            "Le {} est aujourd'hui ou déjà passé : la dictée doit être à venir.",
            ecrire_jour(cette_annee)
        ))),
    }
}

/// Le tableau de la dictée du coffre `reference` : le texte entier, où le coffre posé est le
/// trou à remplir, les coffres en file des trous en attente, et les coffres ouverts de la
/// même séance (créés au même instant) des mots trouvés. `None` si ce coffre n'est pas celui
/// d'une dictée enregistrée.
pub fn tableau(
    depot: &Depot,
    reference: &ExerciceStocke,
) -> Result<Option<Vec<Morceau>>, ErreurDictees> {
    let Some(dictee) = reference
        .exercice
        .donnees
        .get("dictee")
        .and_then(Value::as_i64)
    else {
        return Ok(None);
    };
    let (phrases, _) = phrases_enregistrees(depot, dictee)?;
    let mut trous = HashMap::new();
    for coffre in depot.coffres_de_dictee(dictee)? {
        let etat = match coffre.statut {
            Statut::Pose => Trou::Actif,
            Statut::EnFile => Trou::Attente,
            Statut::Ouvert if coffre.cree_le == reference.cree_le => Trou::Trouve,
            _ => continue,
        };
        let indice = |cle: &str| {
            coffre
                .exercice
                .donnees
                .get(cle)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
        };
        if let (Some(phrase), Some(mot)) = (indice("phrase"), indice("mot")) {
            let position = Position { phrase, mot };
            // Un trou encore à trouver l'emporte sur le même mot déjà trouvé.
            if etat != Trou::Trouve || !trous.contains_key(&position) {
                trous.insert(position, etat);
            }
        }
    }
    Ok(Some(morceaux(&phrases, &trous)))
}
