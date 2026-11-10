//! Dictée préparée, attendu de fin de CM2 : orthographier les mots d'un texte étudié, accords
//! et lettres muettes compris.
//!
//! L'enfant fait des dictées **à trous** : la phrase est écrite, il manque un mot. Ce module
//! prend le texte fourni par un parent (celui que la maîtresse donne à préparer), le découpe en
//! phrases et en mots, et tire les trous. Il n'y a pas de générateur au sens du registre :
//! sans texte, pas d'exercice.
//!
//! **La réponse n'est pas calculée, elle est découpée** : c'est le mot du texte, à sa position.
//! Aucun LLM ne choisit un trou ni n'écrit une réponse.
//!
//! **Écriture unique.** La clé est le mot exactement comme dans le texte, majuscule comprise :
//! « Dans » en début de phrase, « Dubaï », « tours ». Aucune règle ne devine les noms propres.
//! La seule retouche est l'apostrophe : la typographique (« ’ ») devient celle du clavier
//! (« ' »), dans la clé comme dans la phrase affichée.
//!
//! **Mots candidats.** Tout mot d'au moins deux lettres, apostrophe et trait d'union compris
//! (« s'élève », « huit-cent-vingt-huit »). Exclus : plus de 50 caractères (limite de
//! l'enclume), un chiffre, une majuscule accentuée (« Émirats » : difficile à taper).
//!
//! **Ce que voit l'enfant.** La phrase avec `_____` à la place du mot, toujours cinq tirets :
//! la longueur du trou ne dit rien du mot. Sur le panneau, « Dictée : » puis le mot d'avant, le
//! trou et le mot d'après, quand ils tiennent et qu'ils ne sont pas le mot lui-même.

use std::collections::{HashMap, HashSet};

use serde_json::json;

use crate::alea::Alea;
use crate::exercises::base::Exercice;
use crate::minecraft::texte::lignes_panneau;
use crate::saisie::cle_dictee_valide;
use crate::util::{est_espace, longueur, mots as decouper_espaces};

pub const COMPETENCE: &str = "francais/dictee";
/// Le trou, de longueur fixe.
pub const TROU: &str = "_____";
pub const LETTRES_MIN: usize = 2;

/// Texte ou tirage refusé ; le message s'adresse au parent, tel quel.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurDictee(pub String);

/// Un mot dans sa phrase. `debut` et `fin` se comptent en caractères, pas en octets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mot {
    pub texte: String,
    pub debut: usize,
    pub fin: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phrase {
    pub texte: String,
    pub mots: Vec<Mot>,
}

/// Un trou : le mot n°`mot` de la phrase n°`phrase`, tous deux comptés à partir de zéro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Position {
    pub phrase: usize,
    pub mot: usize,
}

fn est_apostrophe(c: char) -> bool {
    matches!(c, '\'' | '’' | 'ʼ' | '‘' | '`' | '´')
}

/// Espaces réduites à une seule, apostrophes ramenées à celle du clavier.
fn normaliser_texte(texte: &str) -> String {
    let apostrophes: String = texte
        .chars()
        .map(|c| if est_apostrophe(c) { '\'' } else { c })
        .collect();
    decouper_espaces(&apostrophes).collect::<Vec<_>>().join(" ")
}

fn dans_un_mot(c: char) -> bool {
    c.is_alphanumeric()
}

fn decouper_mots(phrase: &str) -> Vec<Mot> {
    let caracteres: Vec<char> = phrase.chars().collect();
    let mut mots = Vec::new();
    let mut i = 0;
    while i < caracteres.len() {
        if !dans_un_mot(caracteres[i]) {
            i += 1;
            continue;
        }
        let debut = i;
        while i < caracteres.len() {
            let c = caracteres[i];
            // Apostrophe et trait d'union tiennent au mot s'ils sont entre deux lettres.
            let liaison =
                matches!(c, '\'' | '-') && caracteres.get(i + 1).copied().is_some_and(dans_un_mot);
            if dans_un_mot(c) || liaison {
                i += 1;
            } else {
                break;
            }
        }
        mots.push(Mot {
            texte: caracteres[debut..i].iter().collect(),
            debut,
            fin: i,
        });
    }
    mots
}

fn phrase(texte: &str) -> Option<Phrase> {
    let texte = texte.trim_matches(est_espace).to_owned();
    let mots = decouper_mots(&texte);
    (!mots.is_empty()).then_some(Phrase { texte, mots })
}

/// Découpe un texte en phrases, sur `.`, `!`, `?` et `…` suivis d'une espace ou de la fin.
pub fn decouper(texte: &str) -> Vec<Phrase> {
    let caracteres: Vec<char> = normaliser_texte(texte).chars().collect();
    let mut phrases = Vec::new();
    let mut courante = String::new();
    let mut i = 0;
    while i < caracteres.len() {
        let c = caracteres[i];
        courante.push(c);
        i += 1;
        if !matches!(c, '.' | '!' | '?' | '…') {
            continue;
        }
        // La ponctuation qui se suit (« ?! », « ... ») et les guillemets fermants restent
        // avec la phrase.
        while let Some(&suivant) = caracteres.get(i) {
            if matches!(suivant, '.' | '!' | '?' | '…' | '»' | '"' | '”' | ')') {
                courante.push(suivant);
                i += 1;
            } else if suivant == ' ' && caracteres.get(i + 1) == Some(&'»') {
                courante.push_str(" »");
                i += 2;
            } else {
                break;
            }
        }
        if caracteres.get(i).is_none_or(|&s| s == ' ') {
            phrases.extend(phrase(&std::mem::take(&mut courante)));
        }
    }
    phrases.extend(phrase(&courante));
    phrases
}

/// Une liste de mots à apprendre, sans texte : chaque élément est lu seul.
pub fn decouper_liste(liste: &str) -> Vec<Phrase> {
    liste
        .split([',', ';', '\n'])
        .filter_map(|element| phrase(&normaliser_texte(element)))
        .collect()
}

/// Ce mot peut-il devenir un trou ?
pub fn est_candidat(mot: &str) -> bool {
    let lettres = mot.chars().filter(|c| c.is_alphabetic()).count();
    lettres >= LETTRES_MIN
        && cle_dictee_valide(mot)
        && !mot.chars().any(|c| c.is_uppercase() && !c.is_ascii())
}

/// Tous les trous possibles du texte, dans l'ordre.
pub fn candidats(phrases: &[Phrase]) -> Vec<Position> {
    let mut positions = Vec::new();
    for (p, phrase) in phrases.iter().enumerate() {
        for (m, mot) in phrase.mots.iter().enumerate() {
            if est_candidat(&mot.texte) {
                positions.push(Position { phrase: p, mot: m });
            }
        }
    }
    positions
}

fn mot_a(phrases: &[Phrase], position: Position) -> Option<&Mot> {
    phrases.get(position.phrase)?.mots.get(position.mot)
}

fn meme_mot(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Les mots imposés par le parent qui ne sont pas des trous possibles du texte.
pub fn imposes_absents(phrases: &[Phrase], imposes: &[String]) -> Vec<String> {
    let possibles: Vec<&str> = candidats(phrases)
        .into_iter()
        .filter_map(|p| mot_a(phrases, p))
        .map(|m| m.texte.as_str())
        .collect();
    imposes
        .iter()
        .map(|mot| normaliser_texte(mot))
        .filter(|mot| !possibles.iter().any(|p| meme_mot(p, mot)))
        .collect()
}

/// Tire au plus `n` trous, sans remise.
///
/// Les mots imposés sortent d'abord, puis le hasard, selon `poids` (1 pour une position
/// absente). Un poids nul écarte la position, même imposée : c'est ainsi qu'un mot acquis ou
/// raté la veille ne sort pas. Deux trous ne tombent dans la même phrase que si toutes les
/// phrases encore utilisables en ont déjà un.
pub fn tirer(
    phrases: &[Phrase],
    n: usize,
    alea: &mut Alea,
    poids: &HashMap<Position, f64>,
    imposes: &[String],
) -> Result<Vec<Position>, ErreurDictee> {
    let absents = imposes_absents(phrases, imposes);
    if !absents.is_empty() {
        return Err(ErreurDictee(format!(
            "Pas dans le texte, ou impossible à demander : {}.",
            absents.join(", ")
        )));
    }
    let poids_de = |p: &Position| {
        let valeur = poids.get(p).copied().unwrap_or(1.0);
        if valeur.is_finite() && valeur > 0.0 {
            valeur
        } else {
            0.0
        }
    };
    let est_impose = |p: &Position| {
        mot_a(phrases, *p).is_some_and(|m| {
            imposes
                .iter()
                .any(|i| meme_mot(&normaliser_texte(i), &m.texte))
        })
    };
    let mut restants: Vec<Position> = candidats(phrases)
        .into_iter()
        .filter(|p| poids_de(p) > 0.0)
        .collect();
    let mut tires: Vec<Position> = Vec::new();
    let mut phrases_prises: HashSet<usize> = HashSet::new();
    while tires.len() < n && !restants.is_empty() {
        // Du plus contraint au moins contraint : imposé dans une phrase libre, imposé,
        // phrase libre, puis n'importe lequel.
        let libre = |p: &Position| !phrases_prises.contains(&p.phrase);
        let filtres: [&dyn Fn(&Position) -> bool; 4] = [
            &|p| est_impose(p) && libre(p),
            &|p| est_impose(p),
            &|p| libre(p),
            &|_| true,
        ];
        let choix: Vec<usize> = filtres
            .iter()
            .map(|filtre| {
                (0..restants.len())
                    .filter(|&i| filtre(&restants[i]))
                    .collect::<Vec<_>>()
            })
            .find(|indices| !indices.is_empty())
            .unwrap_or_default();
        let poids_du_choix: Vec<f64> = choix.iter().map(|&i| poids_de(&restants[i])).collect();
        let Some(gagnant) = alea.pondere(&poids_du_choix) else {
            break;
        };
        let position = restants.remove(choix[gagnant]);
        phrases_prises.insert(position.phrase);
        tires.push(position);
    }
    Ok(tires)
}

/// La phrase avec `_____` à la place du mot.
pub fn phrase_a_trou(phrase: &Phrase, mot: &Mot) -> String {
    let avant: String = phrase.texte.chars().take(mot.debut).collect();
    let apres: String = phrase.texte.chars().skip(mot.fin).collect();
    format!("{avant}{TROU}{apres}")
}

fn enonce(phrase: &Phrase, indice: usize) -> String {
    let mot = &phrase.mots[indice];
    // Un voisin identique au mot donnerait la réponse sur le panneau.
    let voisin = |i: Option<usize>| {
        i.and_then(|i| phrase.mots.get(i))
            .map(|m| m.texte.as_str())
            .filter(|v| !meme_mot(v, &mot.texte))
    };
    let avant = voisin(indice.checked_sub(1));
    let apres = voisin(indice.checked_add(1));
    let essais = [(avant, apres), (avant, None), (None, apres), (None, None)];
    for (avant, apres) in essais {
        let texte = ["Dictée :"]
            .into_iter()
            .chain(avant)
            .chain([TROU])
            .chain(apres)
            .collect::<Vec<_>>()
            .join(" ");
        if lignes_panneau(&texte).is_some() {
            return texte;
        }
    }
    format!("Dictée : {TROU}")
}

/// L'exercice d'un trou : la clé est le mot du texte, tel quel.
pub fn exercice(phrases: &[Phrase], position: Position) -> Result<Exercice, ErreurDictee> {
    let inconnu = || ErreurDictee("Ce mot n'est pas dans le texte.".to_owned());
    let phrase = phrases.get(position.phrase).ok_or_else(inconnu)?;
    let mot = phrase.mots.get(position.mot).ok_or_else(inconnu)?;
    if !est_candidat(&mot.texte) {
        return Err(ErreurDictee(format!(
            "« {} » ne peut pas être demandé.",
            mot.texte
        )));
    }
    let serde_json::Value::Object(donnees) = json!({
        "phrase": position.phrase,
        "mot": position.mot,
        "a_trou": phrase_a_trou(phrase, mot),
        "lettres": longueur(&mot.texte),
    }) else {
        return Err(inconnu());
    };
    Ok(Exercice {
        competence: COMPETENCE.to_owned(),
        enonce: enonce(phrase, position.mot),
        reponse: mot.texte.clone(),
        donnees,
    })
}
