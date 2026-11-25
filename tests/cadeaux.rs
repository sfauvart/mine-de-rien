//! Le tirage des cadeaux d'un coffre : pondéré, sans doublon, rejouable.

use std::collections::HashSet;

use mine_de_rien::alea::Alea;
use mine_de_rien::config::{Cadeau, cadeaux_par_defaut};
use mine_de_rien::games::cadeaux::tirer;

fn cadeau(objet: &str, poids: i64) -> Cadeau {
    Cadeau {
        objet: objet.to_owned(),
        nombre: 1,
        poids,
        nom: None,
    }
}

#[test]
fn toujours_le_bon_nombre_de_cadeaux_differents_issus_de_la_liste() {
    let liste = cadeaux_par_defaut();
    let connus: HashSet<&str> = liste.iter().map(|c| c.objet.as_str()).collect();
    for graine in 0..500 {
        for combien in 1..=5 {
            let tires = tirer(&liste, combien, &mut Alea::graine(graine));
            assert_eq!(tires.len(), combien, "graine {graine}");
            let objets: HashSet<&str> = tires.iter().map(|c| c.objet.as_str()).collect();
            assert_eq!(objets.len(), combien, "doublon, graine {graine}");
            assert!(objets.is_subset(&connus), "graine {graine}");
        }
    }
}

#[test]
fn le_cadeau_tire_porte_le_nombre_et_le_nom_de_la_liste() {
    let liste = cadeaux_par_defaut();
    for graine in 0..100 {
        for tire in tirer(&liste, 2, &mut Alea::graine(graine)) {
            let origine = liste.iter().find(|c| c.objet == tire.objet).unwrap();
            assert_eq!(
                (tire.nombre, &tire.nom),
                (origine.nombre, &origine.nom),
                "graine {graine}"
            );
        }
    }
}

#[test]
fn la_meme_graine_donne_le_meme_tirage() {
    let liste = cadeaux_par_defaut();
    for graine in 0..50 {
        assert_eq!(
            tirer(&liste, 3, &mut Alea::graine(graine)),
            tirer(&liste, 3, &mut Alea::graine(graine))
        );
    }
}

#[test]
fn les_tirages_ne_se_ressemblent_pas_tous() {
    let liste = cadeaux_par_defaut();
    let differents: HashSet<Vec<String>> = (0..50)
        .map(|graine| {
            tirer(&liste, 2, &mut Alea::graine(graine))
                .into_iter()
                .map(|c| c.objet)
                .collect()
        })
        .collect();
    assert!(differents.len() > 10, "{}", differents.len());
}

#[test]
fn un_poids_cinq_fois_plus_grand_sort_cinq_fois_plus() {
    let liste = [cadeau("courant", 10), cadeau("rare", 2)];
    let mut alea = Alea::graine(7);
    let (mut courants, mut rares) = (0u32, 0u32);
    for _ in 0..10_000 {
        match tirer(&liste, 1, &mut alea)[0].objet.as_str() {
            "courant" => courants += 1,
            _ => rares += 1,
        }
    }
    let rapport = f64::from(courants) / f64::from(rares);
    assert!((4.0..=6.0).contains(&rapport), "{courants} / {rares}");
}

#[test]
fn une_liste_trop_courte_rend_ce_qu_elle_peut() {
    let liste = [cadeau("a", 1), cadeau("b", 1)];
    assert_eq!(tirer(&liste, 5, &mut Alea::graine(1)).len(), 2);
    assert!(tirer(&[], 2, &mut Alea::graine(1)).is_empty());
}
