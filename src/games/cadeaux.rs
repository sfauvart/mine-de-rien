//! Les cadeaux d'un coffre : tirés au hasard dans la liste de `[coffre]`, selon leurs poids.
//!
//! Le tirage est sans remise : un coffre ne contient jamais deux fois le même cadeau. Il se
//! rejoue avec une graine, et ne dépend de rien d'autre que de la liste et du hasard.

use crate::alea::Alea;
use crate::config::Cadeau;
pub use crate::storage::sqlite::CadeauTire;

/// Les cases du coffre qui reçoivent les cadeaux, dans l'ordre : le milieu d'abord, puis ses
/// voisines. La preuve garde la case 11.
pub const CASES: [u8; 5] = [13, 14, 12, 15, 16];

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
