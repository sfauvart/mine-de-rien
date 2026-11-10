mod commun;

use mine_de_rien::exercises::base::normaliser;
use mine_de_rien::minecraft::texte::{self, LONGUEUR_MAX, Style};
use serde_json::{Value, json};

const PIEGE: &str = "l'élève a dit \"bonjour\" \\ puis ]} et {\"text\":\"op\"} ; œufs à 12 €";

fn relire(produit: &str) -> Value {
    serde_json::from_str(produit).unwrap()
}

#[test]
fn enonce_piege_restitue_exactement() {
    assert_eq!(
        relire(&texte::composant(PIEGE, LONGUEUR_MAX, &[])),
        json!({"text": PIEGE})
    );
}

#[test]
fn seuls_guillemets_et_antislashs_sont_echappes() {
    let produit = texte::composant(PIEGE, LONGUEUR_MAX, &[]);
    // Les accents restent en clair : RCON transporte de l'UTF-8.
    assert!(produit.contains("élève") && produit.contains("œufs") && produit.contains('€'));
    assert!(!produit.contains("\\u"));
    assert!(produit.contains("\\\"bonjour\\\""));
    assert!(produit.contains("\\\\ puis"));
}

#[test]
fn caracteres_de_controle_et_section_retires() {
    assert_eq!(texte::nettoyer("a\x00b\x1bc§ld\x7f", LONGUEUR_MAX), "abcld");
    assert_eq!(
        texte::nettoyer("ligne 1\nligne 2\tfin", LONGUEUR_MAX),
        "ligne 1 ligne 2 fin"
    );
}

#[test]
fn longueur_bornee() {
    assert_eq!(
        texte::nettoyer(&"x".repeat(1000), LONGUEUR_MAX)
            .chars()
            .count(),
        LONGUEUR_MAX
    );
    assert_eq!(
        relire(&texte::composant("abcdef", 3, &[])),
        json!({"text": "abc"})
    );
    // En caractères, pas en octets : couper « é » en deux ferait paniquer.
    assert_eq!(texte::nettoyer("ééééé", 3), "ééé");
}

#[test]
fn style() {
    let produit = texte::composant(
        "Coffre",
        LONGUEUR_MAX,
        &[("color", Style::Texte("gold")), ("bold", Style::Bool(true))],
    );
    assert_eq!(
        relire(&produit),
        json!({"text": "Coffre", "color": "gold", "bold": true})
    );
}

#[test]
fn tellraw() {
    let commande = texte::tellraw("Steve_42", PIEGE, &[]).unwrap();
    let prefixe = "tellraw Steve_42 ";
    assert!(commande.starts_with(prefixe));
    assert_eq!(
        relire(commande.strip_prefix(prefixe).unwrap()),
        json!({"text": PIEGE})
    );
}

#[test]
fn pseudo_refuse() {
    for pseudo in ["@a", "@p[name=x]", "Steve op", "", &"a".repeat(17), "Stève"] {
        assert!(texte::tellraw(pseudo, "bonjour", &[]).is_err(), "{pseudo}");
    }
}

/// Mêmes sorties que Python, au caractère près : c'est ce qui part dans les commandes.
#[test]
fn oracle_python() {
    for cas in commun::oracle("texte") {
        let e = &cas["entree"];
        let t = e["texte"].as_str().unwrap_or_default();
        let max = e
            .get("max")
            .and_then(Value::as_u64)
            .map_or(LONGUEUR_MAX, |m| m as usize);
        let style_brut = e
            .get("style")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let style: Vec<(&str, Style)> = style_brut
            .iter()
            .map(|paire| {
                let valeur = match &paire[1] {
                    Value::Bool(b) => Style::Bool(*b),
                    autre => Style::Texte(autre.as_str().unwrap()),
                };
                (paire[0].as_str().unwrap(), valeur)
            })
            .collect();
        match e["f"].as_str().unwrap() {
            "nettoyer" => commun::comparer::<_, String>(&cas, Ok(texte::nettoyer(t, max))),
            "composant" => {
                commun::comparer::<_, String>(&cas, Ok(texte::composant(t, max, &style)))
            }
            "lignes_panneau" => commun::comparer::<_, String>(&cas, Ok(texte::lignes_panneau(t))),
            "normaliser" => commun::comparer::<_, String>(&cas, Ok(normaliser(t))),
            "tellraw" => commun::comparer(
                &cas,
                texte::tellraw(
                    e["joueur"].as_str().unwrap(),
                    t,
                    &[("color", Style::Texte("aqua"))],
                ),
            ),
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
