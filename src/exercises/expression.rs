//! Calcul d'une expression recopiée du cahier par un parent : `348 ÷ 12`, `3,5 + 1,25`.
//!
//! Attendu de fin de CM2 : les quatre opérations sur les entiers et les décimaux, avec
//! parenthèses. Mine de rien recalcule toujours la réponse : on ne fait confiance ni au parent
//! ni à un LLM pour le résultat.
//!
//! Analyseur par descente récursive sur une liste de jetons. Jamais d'évaluation de code :
//! tout caractère hors des chiffres, de la virgule, du point, des quatre opérations et des
//! parenthèses est refusé. Le calcul est exact, en fractions de grands entiers.
//!
//! Écriture unique de la réponse (le coffre compare caractère pour caractère) : entier, ou
//! décimal à deux chiffres au plus après la virgule, écrit avec une virgule et sans zéro
//! inutile (`3,5`, jamais `3,50` ni `3.5`). Un résultat qui ne s'écrit pas ainsi (`10 ÷ 3`), ou
//! négatif, est refusé : ce n'est pas un exercice de coffre.
//!
//! Seuls les chiffres de `0` à `9` sont des chiffres. La version Python acceptait aussi les
//! chiffres d'autres écritures (arabes-indiens, etc.) : ici, ils sont refusés.

use std::collections::BTreeSet;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, Zero};
use serde_json::{Map, Value};

use crate::exercises::base::Exercice;
use crate::minecraft::texte::lignes_panneau;
use crate::util::{est_espace, longueur, strip};

/// Garde-fou avant analyse ; l'énoncé normalisé doit en plus tenir sur le panneau.
pub const LONGUEUR_MAX: usize = 60;
pub const CHIFFRES_MAX: usize = 9;

pub const COMPETENCE_MELANGEE: &str = "maths/calcul";

/// Expression refusée ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurExpression(pub String);

fn refus<T>(message: impl Into<String>) -> Result<T, ErreurExpression> {
    Err(ErreurExpression(message.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Operateur {
    Plus,
    Moins,
    Fois,
    Divise,
}

impl Operateur {
    fn lire(c: char) -> Option<Self> {
        match c {
            '+' => Some(Self::Plus),
            '-' => Some(Self::Moins),
            '×' | 'x' | 'X' | '*' => Some(Self::Fois),
            '÷' | ':' | '/' => Some(Self::Divise),
            _ => None,
        }
    }

    fn texte(self) -> &'static str {
        match self {
            Self::Plus => "+",
            Self::Moins => "-",
            Self::Fois => "×",
            Self::Divise => "÷",
        }
    }

    fn competence(self) -> &'static str {
        match self {
            Self::Plus => "maths/addition-posee",
            Self::Moins => "maths/soustraction-posee",
            Self::Fois => "maths/multiplication-posee",
            Self::Divise => "maths/division-posee",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Jeton {
    Nombre { texte: String, valeur: BigRational },
    Op(Operateur),
    Ouvrante,
    Fermante,
}

impl Jeton {
    fn texte(&self) -> &str {
        match self {
            Self::Nombre { texte, .. } => texte,
            Self::Op(op) => op.texte(),
            Self::Ouvrante => "(",
            Self::Fermante => ")",
        }
    }
}

/// Retire « = » ou « = ? » en fin de texte, tels qu'on les recopie du cahier.
fn sans_egal_final(texte: &str) -> &str {
    let sans_fin = texte.trim_end_matches(est_espace);
    let sans_fin = sans_fin
        .strip_suffix('?')
        .unwrap_or(sans_fin)
        .trim_end_matches(est_espace);
    match sans_fin.strip_suffix('=') {
        Some(avant) => avant.trim_end_matches(est_espace),
        None => texte,
    }
}

fn nombre(entier: &str, decimales: &str) -> BigRational {
    let chiffres = format!("{entier}{decimales}");
    let numerateur: BigInt = chiffres.parse().unwrap_or_default();
    BigRational::new(numerateur, BigInt::from(10).pow(decimales.len() as u32))
}

fn decouper(texte: &str) -> Result<Vec<Jeton>, ErreurExpression> {
    let texte = sans_egal_final(strip(texte));
    if longueur(texte) > LONGUEUR_MAX {
        return refus(format!(
            "Expression trop longue ({LONGUEUR_MAX} caractères au plus)."
        ));
    }
    let caracteres: Vec<char> = texte.chars().collect();
    let mut jetons = Vec::new();
    let mut i = 0;
    while i < caracteres.len() {
        while i < caracteres.len() && est_espace(caracteres[i]) {
            i += 1;
        }
        let Some(&c) = caracteres.get(i) else { break };
        if c.is_ascii_digit() {
            let debut = i;
            while i < caracteres.len() && caracteres[i].is_ascii_digit() {
                i += 1;
            }
            let entier: String = caracteres[debut..i].iter().collect();
            let mut decimales = String::new();
            if matches!(caracteres.get(i), Some('.' | ','))
                && caracteres.get(i + 1).is_some_and(char::is_ascii_digit)
            {
                i += 1;
                while i < caracteres.len() && caracteres[i].is_ascii_digit() {
                    decimales.push(caracteres[i]);
                    i += 1;
                }
            }
            if entier.len() > CHIFFRES_MAX {
                return refus("Nombre trop grand pour un exercice de CM2.");
            }
            let valeur = nombre(&entier, &decimales);
            let texte = if decimales.is_empty() {
                entier
            } else {
                format!("{entier},{decimales}")
            };
            jetons.push(Jeton::Nombre { texte, valeur });
        } else if c == '(' {
            jetons.push(Jeton::Ouvrante);
            i += 1;
        } else if c == ')' {
            jetons.push(Jeton::Fermante);
            i += 1;
        } else if let Some(op) = Operateur::lire(c) {
            jetons.push(Jeton::Op(op));
            i += 1;
        } else {
            return refus(format!("Caractère non reconnu : « {c} »."));
        }
    }
    if jetons.is_empty() {
        return refus("Expression vide.");
    }
    Ok(jetons)
}

/// ```text
/// expression := terme (('+' | '-') terme)*
/// terme      := facteur (('×' | '÷') facteur)*
/// facteur    := nombre | '(' expression ')'
/// ```
struct Analyseur<'a> {
    jetons: &'a [Jeton],
    i: usize,
}

impl Analyseur<'_> {
    fn courant(&self) -> Option<&Jeton> {
        self.jetons.get(self.i)
    }

    fn analyser(&mut self) -> Result<BigRational, ErreurExpression> {
        let valeur = self.expression()?;
        match self.courant() {
            Some(jeton) => refus(format!(
                "Expression mal formée près de « {} ».",
                jeton.texte()
            )),
            None => Ok(valeur),
        }
    }

    fn expression(&mut self) -> Result<BigRational, ErreurExpression> {
        let mut valeur = self.terme()?;
        while let Some(Jeton::Op(op @ (Operateur::Plus | Operateur::Moins))) = self.courant() {
            let op = *op;
            self.i += 1;
            let droite = self.terme()?;
            valeur = if op == Operateur::Plus {
                valeur + droite
            } else {
                valeur - droite
            };
        }
        Ok(valeur)
    }

    fn terme(&mut self) -> Result<BigRational, ErreurExpression> {
        let mut valeur = self.facteur()?;
        while let Some(Jeton::Op(op @ (Operateur::Fois | Operateur::Divise))) = self.courant() {
            let op = *op;
            self.i += 1;
            let droite = self.facteur()?;
            if op == Operateur::Fois {
                valeur *= droite;
            } else if droite.is_zero() {
                return refus("Division par zéro.");
            } else {
                valeur /= droite;
            }
        }
        Ok(valeur)
    }

    fn facteur(&mut self) -> Result<BigRational, ErreurExpression> {
        match self.courant() {
            None => refus("Expression incomplète."),
            Some(Jeton::Nombre { valeur, .. }) => {
                let valeur = valeur.clone();
                self.i += 1;
                Ok(valeur)
            }
            Some(Jeton::Ouvrante) => {
                self.i += 1;
                let valeur = self.expression()?;
                if self.courant() != Some(&Jeton::Fermante) {
                    return refus("Parenthèse non fermée.");
                }
                self.i += 1;
                Ok(valeur)
            }
            Some(jeton) => refus(format!(
                "Expression mal formée près de « {} ».",
                jeton.texte()
            )),
        }
    }
}

/// Écriture unique : entier, ou décimal à deux chiffres au plus, avec une virgule.
///
/// Refuse une valeur qui ne s'écrit pas ainsi, ou négative.
pub fn ecrire_reponse(valeur: &BigRational) -> Result<String, ErreurExpression> {
    if valeur.is_negative() {
        return refus("Le résultat est négatif : ce n'est pas un exercice de CM2.");
    }
    let centiemes = valeur * BigRational::from_integer(BigInt::from(100));
    if !centiemes.is_integer() {
        return refus(
            "Le résultat ne tombe pas juste (plus de deux chiffres après la virgule) : \
             le coffre ne saurait pas quelle réponse attendre.",
        );
    }
    let centiemes = centiemes.to_integer();
    let cent = BigInt::from(100);
    let (entier, decimales) = (&centiemes / &cent, &centiemes % &cent);
    if decimales.is_zero() {
        return Ok(entier.to_string());
    }
    let ecrit = format!("{entier},{:0>2}", decimales.to_string());
    Ok(ecrit.trim_end_matches('0').to_owned())
}

/// L'énoncé normalisé. Une parenthèse ouvrante ne colle au nombre suivant que si elle-même a
/// été écrite sans espace devant (en tête, ou après une autre parenthèse collée) : après un
/// opérateur, on obtient « × ( 3 + 4) ». C'est l'écriture de la version Python, gardée telle
/// quelle pour que les énoncés ne changent pas d'une version à l'autre.
fn enonce(jetons: &[Jeton]) -> String {
    let mut sortie = String::new();
    let mut ouvrante_collee = false;
    for (i, jeton) in jetons.iter().enumerate() {
        let colle = *jeton == Jeton::Fermante || ouvrante_collee || i == 0;
        if !colle {
            sortie.push(' ');
        }
        sortie.push_str(jeton.texte());
        ouvrante_collee = *jeton == Jeton::Ouvrante && colle;
    }
    sortie + " = ?"
}

/// Transforme une expression en `Exercice`, réponse recalculée, ou la refuse.
pub fn calculer(texte: &str) -> Result<Exercice, ErreurExpression> {
    let jetons = decouper(texte)?;
    let operateurs: BTreeSet<Operateur> = jetons
        .iter()
        .filter_map(|jeton| {
            if let Jeton::Op(op) = jeton {
                Some(*op)
            } else {
                None
            }
        })
        .collect();
    if operateurs.is_empty() {
        return refus("Il manque une opération (+, -, ×, ÷).");
    }
    let valeur = Analyseur {
        jetons: &jetons,
        i: 0,
    }
    .analyser()?;
    let reponse = ecrire_reponse(&valeur)?;
    let enonce = enonce(&jetons);
    if lignes_panneau(&enonce).is_none() {
        return refus("Expression trop longue pour tenir sur le panneau (4 lignes de 15).");
    }
    let competence = match operateurs.first() {
        Some(op) if operateurs.len() == 1 => op.competence(),
        _ => COMPETENCE_MELANGEE,
    };
    let mut donnees = Map::new();
    let expression = enonce.strip_suffix(" = ?").unwrap_or(&enonce).to_owned();
    donnees.insert("expression".to_owned(), Value::String(expression));
    Ok(Exercice {
        competence: competence.to_owned(),
        enonce,
        reponse,
        donnees,
    })
}
