//! Photo d'une fiche → exercices vérifiés, sans Telegram.
//!
//! Le LLM local **recopie** les lignes imprimées : calculs et mots anglais. Il ne calcule rien,
//! et ce qu'il écrit après un `=` est jeté sans être lu. Chaque ligne devient un `Exercice` par
//! le code existant, ou elle est rejetée avec sa raison :
//!
//! - « Double de N » et « Moitié de N » deviennent `N x 2` et `N ÷ 2`, recalculés par
//!   l'analyseur, avec l'énoncé de la fiche (« Double de 17 = ? ») ;
//! - tout autre calcul passe par l'analyseur d'expressions (`exercises/expression.rs`) ;
//! - un mot passe par le lexique fermé du vocabulaire anglais.
//!
//! Essais du 2026-11-08 (spec `2026-11-photo-du-cahier`) : une page droite est bien lue, mais
//! une double page envoyée d'un coup fait inventer des lignes (« Moitié de 3, 5, 7… ») ou en
//! fait sauter. Une image plus large que haute est donc coupée en deux moitiés qui se
//! recouvrent, lues séparément ; les doublons sont retirés. Le modèle ne sait pas dire dans quel
//! sens est une photo : le redressement est l'affaire du parent.
//!
//! Les photos ne sont jamais écrites sur le disque, et ne partent que vers un LLM local.

use std::collections::HashSet;
use std::io::Cursor;
use std::sync::LazyLock;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageReader};
use regex::Regex;
use serde_json::{Map, Value};

use crate::adapters::llm::base::{ErreurLlm, Llm, lire_json};
use crate::alea::Alea;
use crate::exercises::anglais::vocabulaire;
use crate::exercises::base::{Exercice, normaliser};
use crate::exercises::expression::{ErreurExpression, calculer};
use crate::util::{capitaliser, debut, sans_milliers, str_python, strip};

pub const COTE_MAX: u32 = 1536;
pub const MOITIES: [(f64, f64); 2] = [(0.0, 0.53), (0.47, 1.0)];
pub const COMPETENCE_DOUBLES: &str = "maths/doubles-moities";
pub const HORS_LEXIQUE: &str = "hors lexique";

pub const SYSTEME: &str = "Tu transcris fidèlement des fiches d'exercices photographiées. Tu ne \
    calcules rien, tu ne prolonges aucune suite, tu ne remets rien dans un ordre logique, tu \
    ignores l'écriture manuscrite. Tu ne recopies que ce qui est imprimé et visible.";
// Aucun exemple chiffré ni mot d'exemple : sur une photo qu'il ne lit pas, le modèle recopie
// les exemples de la consigne comme s'ils étaient sur la fiche (essai du 2026-11-08).
pub const CONSIGNE: &str = "Recopie chaque ligne de calcul imprimée (opérations, tables, lignes \
    « Double de … » ou « Moitié de … ») et chaque mot anglais imprimé, dans l'ordre de la photo. \
    Pour un calcul, recopie seulement ce qui est avant le signe =. S'il n'y a rien de tel, \
    renvoie une liste vide. Réponds en JSON : \
    {\"lignes\": [{\"genre\": \"calcul\" ou \"mot\", \"texte\": \"...\"}]}";

static DOUBLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(double|moitie)\s*(?:de|d')\s*([0-9]+(?:[.,][0-9]+)?)$").unwrap()
});
static ESPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// Espace, espace insécable, espace fine insécable : les séparateurs de milliers des fiches.
const SEPARATEURS_DE_MILLIERS: [char; 3] = [' ', '\u{a0}', '\u{202f}'];

/// Photo illisible ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurPhoto(pub String);

/// Ce qui empêche de lire une photo : l'image elle-même, ou le LLM.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurLecture {
    #[error("{0}")]
    Photo(#[from] ErreurPhoto),
    #[error("{0}")]
    Llm(#[from] ErreurLlm),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejet {
    pub texte: String,
    pub raison: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Lecture {
    pub exercices: Vec<Exercice>,
    pub rejets: Vec<Rejet>,
}

/// Les dimensions d'une image réduite pour tenir dans un carré de `cote`, sans l'agrandir.
/// Même arrondi que `Image.thumbnail` de Pillow, pour que les morceaux aient la même taille
/// qu'avec la version Python.
fn dimensions_reduites(largeur: u32, hauteur: u32, cote: u32) -> (u32, u32) {
    if largeur <= cote && hauteur <= cote {
        return (largeur, hauteur);
    }
    let (l, h, c) = (f64::from(largeur), f64::from(hauteur), f64::from(cote));
    let aspect = l / h;
    // Des deux entiers qui encadrent `nombre`, celui qui respecte le mieux les proportions.
    let arrondi = |nombre: f64, ecart: &dyn Fn(f64) -> f64| -> u32 {
        let (bas, haut) = (nombre.floor(), nombre.ceil());
        let choisi = if ecart(bas) <= ecart(haut) { bas } else { haut };
        choisi.max(1.0) as u32
    };
    if 1.0 >= aspect {
        (
            arrondi(c * aspect, &|n| (aspect - n / c).abs()),
            cote.min(hauteur),
        )
    } else {
        (
            cote.min(largeur),
            arrondi(c / aspect, &|n| {
                if n == 0.0 {
                    0.0
                } else {
                    (aspect - c / n).abs()
                }
            }),
        )
    }
}

fn ouvrir(octets: &[u8]) -> Option<DynamicImage> {
    let mut decodeur = ImageReader::new(Cursor::new(octets))
        .with_guessed_format()
        .ok()?
        .into_decoder()
        .ok()?;
    let orientation = decodeur.orientation().ok();
    let mut image = DynamicImage::from_decoder(decodeur).ok()?;
    if let Some(orientation) = orientation {
        image.apply_orientation(orientation);
    }
    Some(DynamicImage::ImageRgb8(image.to_rgb8()))
}

/// Redresse selon l'EXIF, coupe une double page en deux, réduit et encode en JPEG.
pub fn preparer(octets: &[u8]) -> Result<Vec<Vec<u8>>, ErreurPhoto> {
    let illisible = || ErreurPhoto("Je n'arrive pas à ouvrir cette image.".to_owned());
    let image = ouvrir(octets).ok_or_else(illisible)?;
    let (largeur, hauteur) = (image.width(), image.height());
    let morceaux: Vec<DynamicImage> = if largeur > hauteur {
        MOITIES
            .iter()
            .map(|(depart, fin)| {
                let gauche = (depart * f64::from(largeur)) as u32;
                let droite = (fin * f64::from(largeur)) as u32;
                image.crop_imm(gauche, 0, droite - gauche, hauteur)
            })
            .collect()
    } else {
        vec![image]
    };
    let mut sorties = Vec::with_capacity(morceaux.len());
    for morceau in morceaux {
        let (l, h) = dimensions_reduites(morceau.width(), morceau.height(), COTE_MAX);
        let reduit = if (l, h) == (morceau.width(), morceau.height()) {
            morceau
        } else {
            morceau.resize_exact(l, h, FilterType::CatmullRom)
        };
        let mut tampon = Vec::new();
        reduit
            .write_with_encoder(JpegEncoder::new_with_quality(&mut tampon, 85))
            .map_err(|_| illisible())?;
        sorties.push(tampon);
    }
    Ok(sorties)
}

/// Accents retirés pour reconnaître « Moitié », espaces simples, sans toucher aux nombres.
fn simplifier(texte: &str) -> String {
    let sans_accents = texte.replace('é', "e").replace('É', "E");
    strip(&ESPACES.replace_all(&sans_accents, " ")).to_owned()
}

/// Un calcul recopié d'une fiche → exercice dont la réponse est recalculée ici.
pub fn calcul(texte: &str) -> Result<Exercice, ErreurExpression> {
    let avant_egal = texte.split('=').next().unwrap_or_default();
    let tirets: String = avant_egal
        .chars()
        .map(|c| {
            if matches!(c, '–' | '—' | '−') {
                '-'
            } else {
                c
            }
        })
        .collect();
    let texte = sans_milliers(strip(&tirets), &SEPARATEURS_DE_MILLIERS);
    if let Some(m) = DOUBLE.captures(&simplifier(&texte)) {
        let double = normaliser(&m[1]) == "double";
        let nombre = &m[2];
        let ex = calculer(&if double {
            format!("{nombre} x 2")
        } else {
            format!("{nombre} ÷ 2")
        })?;
        let (mot, famille) = if double {
            ("Double", "doubles")
        } else {
            ("Moitié", "moities")
        };
        let mut donnees = Map::new();
        donnees.insert("famille".to_owned(), Value::String(famille.to_owned()));
        donnees.insert("nombre".to_owned(), Value::String(nombre.to_owned()));
        return Ok(Exercice {
            competence: COMPETENCE_DOUBLES.to_owned(),
            enonce: format!("{mot} de {nombre} = ?"),
            reponse: ex.reponse,
            donnees,
        });
    }
    let mut ex = calculer(&texte)?;
    ex.donnees
        .insert("famille".to_owned(), Value::String("calculs".to_owned()));
    Ok(ex)
}

fn texte_de<'a>(ex: &'a Exercice, cle: &str) -> Option<&'a str> {
    ex.donnees.get(cle).and_then(Value::as_str)
}

/// Réponses JSON du LLM (une par morceau d'image) → exercices validés et rejets.
pub fn interpreter(reponses: &[Map<String, Value>], alea: &mut Alea) -> Result<Lecture, ErreurLlm> {
    let mut lecture = Lecture::default();
    let mut vus: HashSet<String> = HashSet::new();
    for reponse in reponses {
        let Some(Value::Array(lignes)) = reponse.get("lignes") else {
            return Err(ErreurLlm(
                "Le LLM n'a pas renvoyé de liste de lignes.".to_owned(),
            ));
        };
        for ligne in lignes {
            let Some(texte) = ligne
                .as_object()
                .and_then(|l| l.get("texte"))
                .and_then(Value::as_str)
            else {
                lecture.rejets.push(Rejet {
                    texte: debut(&str_python(ligne), 60).to_owned(),
                    raison: "ligne mal formée".to_owned(),
                });
                continue;
            };
            let texte = debut(strip(texte), 80);
            if texte.is_empty() {
                continue;
            }
            let (ex, cle) = if ligne.get("genre").and_then(Value::as_str) == Some("mot") {
                let Some(ex) = vocabulaire::exercice_pour(texte, alea) else {
                    lecture.rejets.push(Rejet {
                        texte: texte.to_owned(),
                        raison: HORS_LEXIQUE.to_owned(),
                    });
                    continue;
                };
                let cle = format!("mot:{}", texte_de(&ex, "anglais").unwrap_or_default());
                (ex, cle)
            } else {
                match calcul(texte) {
                    Ok(ex) => {
                        let cle = format!("calcul:{}", ex.enonce);
                        (ex, cle)
                    }
                    Err(e) => {
                        lecture.rejets.push(Rejet {
                            texte: texte.to_owned(),
                            raison: e.to_string(),
                        });
                        continue;
                    }
                }
            };
            if vus.insert(cle) {
                lecture.exercices.push(ex);
            }
        }
    }
    Ok(lecture)
}

/// Lit une photo avec le LLM local. Refuse un LLM distant sans l'appeler.
pub async fn lire(
    llm: &dyn Llm,
    octets: &[u8],
    consigne_parent: &str,
    alea: &mut Alea,
) -> Result<Lecture, ErreurLecture> {
    if llm.distant() {
        return Err(ErreurLlm(
            "La lecture de photo demande un LLM local ([llm] adaptateur = \
             \"openai-compatible\") : une photo ne part jamais vers un LLM distant."
                .to_owned(),
        )
        .into());
    }
    let mut consigne = CONSIGNE.to_owned();
    let partie = strip(consigne_parent);
    if !partie.is_empty() {
        consigne += &format!(
            " Ne regarde que cette partie de la page : {}.",
            debut(partie, 100)
        );
    }
    let mut reponses = Vec::new();
    for morceau in preparer(octets)? {
        let texte = llm
            .completer(SYSTEME, &consigne, Some(&morceau), true)
            .await?;
        reponses.push(lire_json(&texte)?);
    }
    Ok(interpreter(&reponses, alea)?)
}

/// N exercices au hasard, sans remise, dans l'ordre de la fiche.
pub fn tirer(lecture: &Lecture, n: usize, alea: &mut Alea) -> Vec<Exercice> {
    let tous: Vec<usize> = (0..lecture.exercices.len()).collect();
    let mut indices = alea.echantillon(&tous, n.min(tous.len()));
    indices.sort_unstable();
    indices
        .into_iter()
        .map(|i| lecture.exercices[i].clone())
        .collect()
}

fn nom_famille(famille: &str) -> String {
    match famille {
        "doubles" => "Doubles de".to_owned(),
        "moities" => "Moitiés de".to_owned(),
        "calculs" => "Calculs".to_owned(),
        "jours" => "Jours".to_owned(),
        "mois" => "Mois".to_owned(),
        "nombres" => "Nombres".to_owned(),
        "couleurs" => "Couleurs".to_owned(),
        "salutations" => "Salutations".to_owned(),
        autre => capitaliser(autre),
    }
}

fn etiquette(ex: &Exercice) -> String {
    texte_de(ex, "nombre")
        .or_else(|| texte_de(ex, "anglais"))
        .or_else(|| texte_de(ex, "expression"))
        .unwrap_or(&ex.enonce)
        .to_owned()
}

/// Ce qui a été lu, groupé par famille, puis les rejets : pour que le parent voie un trou.
pub fn resumer(lecture: &Lecture) -> Vec<String> {
    let mut groupes: Vec<(&str, Vec<String>)> = Vec::new();
    for ex in &lecture.exercices {
        let famille = texte_de(ex, "famille").unwrap_or("calculs");
        match groupes.iter_mut().find(|(nom, _)| *nom == famille) {
            Some((_, etiquettes)) => etiquettes.push(etiquette(ex)),
            None => groupes.push((famille, vec![etiquette(ex)])),
        }
    }
    let mut lignes = vec![format!("J'ai lu {} exercice(s) :", lecture.exercices.len())];
    for (famille, etiquettes) in &groupes {
        lignes.push(format!(
            "• {} ({}) : {}",
            nom_famille(famille),
            etiquettes.len(),
            etiquettes.join(", ")
        ));
    }
    // Un mot hors lexique est le plus souvent un titre ou une consigne : on le compte. Un calcul
    // écarté peut cacher une ligne mal lue : on le montre.
    let mots = lecture
        .rejets
        .iter()
        .filter(|r| r.raison == HORS_LEXIQUE)
        .count();
    let calculs: Vec<&Rejet> = lecture
        .rejets
        .iter()
        .filter(|r| r.raison != HORS_LEXIQUE)
        .collect();
    if !calculs.is_empty() {
        lignes.push(format!("Calculs écartés ({}) :", calculs.len()));
        lignes.extend(
            calculs
                .iter()
                .take(10)
                .map(|r| format!("• « {} » : {}", r.texte, r.raison)),
        );
        if calculs.len() > 10 {
            lignes.push(format!("• … et {} autre(s)", calculs.len() - 10));
        }
    }
    if mots > 0 {
        lignes.push(format!(
            "{mots} texte(s) hors lexique ignoré(s) (titres, consignes…)."
        ));
    }
    lignes
}
