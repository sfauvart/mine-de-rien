//! Les cadeaux d'un coffre : tirés au hasard dans la liste de `[coffre]`, selon leurs poids.
//!
//! Le tirage est sans remise : un coffre ne contient jamais deux fois le même cadeau. Il se
//! rejoue avec une graine, et ne dépend de rien d'autre que de la liste et du hasard.

use serde::{Deserialize, Serialize};

use crate::alea::Alea;
use crate::config::Cadeau;

/// Un cadeau sorti pour un coffre : ce qu'on pose dans le jeu, et son nom pour les parents.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct CadeauTire {
    pub objet: String,
    pub nombre: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
}

/// `combien` cadeaux différents de `liste` : deux fois plus de chances pour un poids double.
/// Une liste trop courte rend ce qu'elle peut.
pub fn tirer(liste: &[Cadeau], combien: usize, alea: &mut Alea) -> Vec<CadeauTire> {
    let mut poids: Vec<f64> = liste.iter().map(|cadeau| cadeau.poids as f64).collect();
    let mut tires = Vec::with_capacity(combien.min(liste.len()));
    while tires.len() < combien {
        let Some(i) = alea.pondere(&poids) else {
            break;
        };
        // Déjà sorti : il ne sortira plus pour ce coffre.
        poids[i] = 0.0;
        let cadeau = &liste[i];
        tires.push(CadeauTire {
            objet: cadeau.objet.clone(),
            nombre: cadeau.nombre,
            nom: cadeau.nom.clone(),
        });
    }
    tires
}
