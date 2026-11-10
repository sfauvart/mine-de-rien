//! Division euclidienne, attendu de fin de CM2 : diviseur à un chiffre (niveau 1) puis à deux
//! chiffres (niveau 2). Le niveau 1 tombe juste pour que la réponse soit une clé simple.

use serde_json::json;

use crate::alea::Alea;
use crate::exercises::base::Exercice;

pub const COMPETENCE: &str = "maths/division-posee";

pub fn generer(alea: &mut Alea, niveau: u32) -> Exercice {
    let (diviseur, quotient, reste) = if niveau <= 1 {
        (alea.entier(2, 9), alea.entier(12, 99), 0)
    } else {
        let diviseur = alea.entier(11, 25);
        (diviseur, alea.entier(12, 60), alea.entier(0, diviseur - 1))
    };
    let dividende = diviseur * quotient + reste;
    let (enonce, reponse) = if reste == 0 {
        (
            format!("{dividende} ÷ {diviseur} = ?"),
            quotient.to_string(),
        )
    } else {
        (
            format!("{dividende} ÷ {diviseur} : quotient et reste ?"),
            format!("{quotient} r {reste}"),
        )
    };
    let donnees = json!({
        "dividende": dividende, "diviseur": diviseur, "quotient": quotient, "reste": reste,
    });
    Exercice {
        competence: COMPETENCE.to_owned(),
        enonce,
        reponse,
        donnees: donnees.as_object().cloned().unwrap_or_default(),
    }
}
