//! Indices par paliers pour un coffre, calculés ici, sans jamais la réponse.
//!
//! Pixel (spec `2026-11-aide-dans-le-chat`) donne le palier suivant à chaque demande, puis
//! répète le dernier. Un palier aide à trouver la méthode, de plus en plus précisément ; le
//! dernier peut aller jusqu'aux calculs intermédiaires, mais **jamais jusqu'au résultat**.
//!
//! Chaque palier est construit à partir de l'`Exercice` stocké (énoncé, réponse, `donnees`),
//! puis passé au filtre `contient_la_reponse` : un palier qui contiendrait la réponse, sous une
//! forme que l'enclume accepterait ou que l'enfant n'aurait plus qu'à recopier, est retiré.
//! S'il ne reste rien, l'indice de secours ne contient aucun nombre.
//!
//! Le même filtre sert à Pixel sur les phrases du LLM.
//!
//! Familles :
//! - doubles et moitiés (`maths/doubles-moities`) : définition, décomposition, morceaux ;
//! - calculs à deux nombres (+, −, ×, ÷), saisis par un parent, lus sur une photo ou tirés
//!   par le générateur de division : méthode, décomposition, calculs intermédiaires ;
//! - calcul avec plusieurs opérations : l'ordre des priorités ;
//! - vocabulaire anglais : sens, première lettre et longueur, repère (rang, objet, moment) ;
//! - tout le reste : relire le panneau.

use std::collections::HashMap;
use std::sync::LazyLock;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{ToPrimitive, Zero};
use regex::Regex;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

use crate::exercises::anglais::vocabulaire;
use crate::exercises::base::Exercice;
use crate::exercises::expression::{ErreurExpression, ecrire_reponse};
use crate::util::{debut, longueur, mots as mots_python, sans_milliers, strip};

pub const LONGUEUR_MAX: usize = 150;
pub const SECOURS: &str =
    "Relis bien le panneau, puis essaie sur l'enclume : chaque essai coûte de l'expérience.";

/// Les repères du dernier indice d'anglais, tels qu'ils étaient à la compilation.
// Chemin provisoire : le fichier rejoint `src/exercises/anglais/donnees/` quand le Python part.
const REPERES: &str = include_str!("../mine_de_rien/exercises/anglais/donnees/indices.toml");

static BINAIRE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([0-9]+(?:[.,][0-9]+)?)\s*([-+×÷])\s*([0-9]+(?:[.,][0-9]+)?)\s*$").unwrap()
});

type Fraction = BigRational;

/// Un indice ne peut pas être écrit pour cet exercice : il n'en aura pas, voilà tout.
struct Abandon;

impl From<ErreurExpression> for Abandon {
    fn from(_: ErreurExpression) -> Self {
        Abandon
    }
}

type Paliers = Result<Vec<String>, Abandon>;

fn entier(n: i64) -> Fraction {
    Fraction::from_integer(BigInt::from(n))
}

// --- Le filtre -------------------------------------------------------------------------

/// Minuscules, sans accents, découpé en mots et en nombres (virgule décimale gardée).
fn mots(texte: &str) -> Vec<String> {
    let sans_accents: String = texte
        .nfkd()
        .filter(char::is_ascii)
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let c: Vec<char> = sans_milliers(&sans_accents, &[' ']).chars().collect();
    let mut sortie = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let depart = i;
        if c[i].is_ascii_digit() {
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            if matches!(c.get(i), Some('.' | ',')) && c.get(i + 1).is_some_and(char::is_ascii_digit)
            {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
        } else if c[i].is_ascii_lowercase() {
            while i < c.len() && c[i].is_ascii_lowercase() {
                i += 1;
            }
        } else {
            i += 1;
            continue;
        }
        sortie.push(c[depart..i].iter().collect());
    }
    sortie
}

fn contient_suite(mots: &[String], suite: &[String]) -> bool {
    !suite.is_empty() && mots.windows(suite.len()).any(|fenetre| fenetre == suite)
}

/// Un nombre écrit en chiffres, avec virgule ou point : sa valeur exacte. `None` sinon.
fn lire_nombre(texte: &str) -> Option<Fraction> {
    let (partie_entiere, decimales) = match texte.split_once(['.', ',']) {
        Some((e, d)) => (e, d),
        None => (texte, ""),
    };
    let chiffres = |t: &str| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit());
    if !chiffres(partie_entiere) || (texte.len() != partie_entiere.len() && !chiffres(decimales)) {
        return None;
    }
    let numerateur: BigInt = format!("{partie_entiere}{decimales}").parse().ok()?;
    Some(Fraction::new(
        numerateur,
        BigInt::from(10).pow(decimales.len() as u32),
    ))
}

fn valeur(texte: &str) -> Option<Fraction> {
    lire_nombre(strip(texte))
}

const UNITES: [&str; 20] = [
    "zero", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix", "onze",
    "douze", "treize", "quatorze", "quinze", "seize", "dix-sept", "dix-huit", "dix-neuf",
];

fn dizaine(d: u32) -> &'static str {
    match d {
        2 => "vingt",
        3 => "trente",
        4 => "quarante",
        5 => "cinquante",
        _ => "soixante",
    }
}

fn moins_de_cent(n: u32) -> String {
    if n < 20 {
        return UNITES[n as usize].to_owned();
    }
    if n < 70 {
        let (d, u) = (n / 10, n % 10);
        return match u {
            0 => dizaine(d).to_owned(),
            1 => format!("{}-et-un", dizaine(d)),
            _ => format!("{}-{}", dizaine(d), UNITES[u as usize]),
        };
    }
    if n < 80 {
        return if n == 71 {
            "soixante-et-onze".to_owned()
        } else {
            format!("soixante-{}", UNITES[(n - 60) as usize])
        };
    }
    if n == 80 {
        return "quatre-vingts".to_owned();
    }
    format!("quatre-vingt-{}", UNITES[(n - 80) as usize])
}

fn moins_de_mille(n: u32) -> String {
    let (c, reste) = (n / 100, n % 100);
    if c == 0 {
        return moins_de_cent(reste);
    }
    let tete = if c == 1 {
        "cent".to_owned()
    } else {
        format!(
            "{}-cent{}",
            UNITES[c as usize],
            if reste == 0 { "s" } else { "" }
        )
    };
    if reste == 0 {
        tete
    } else {
        format!("{tete}-{}", moins_de_cent(reste))
    }
}

/// Un entier de 0 à 999 999 en lettres, avec des tirets partout (rectifications de 1990).
/// Sert seulement à reconnaître la réponse écrite en lettres : la comparaison se fait mot à
/// mot, les tirets et le « et » de « vingt et un » ne comptent donc pas.
///
/// `None` au-delà de 999 999.
pub fn en_lettres(n: u32) -> Option<String> {
    if n >= 1_000_000 {
        return None;
    }
    let (milliers, reste) = (n / 1000, n % 1000);
    if milliers == 0 {
        return Some(moins_de_mille(reste));
    }
    let tete = if milliers == 1 {
        "mille".to_owned()
    } else {
        format!("{}-mille", moins_de_mille(milliers))
    };
    Some(if reste == 0 {
        tete
    } else {
        format!("{tete}-{}", moins_de_mille(reste))
    })
}

fn suite_lettres(n: u32) -> Vec<String> {
    let lettres = en_lettres(n).unwrap_or_default().replace('-', " ");
    mots(&lettres).into_iter().filter(|m| m != "et").collect()
}

fn texte_de<'a>(ex: &'a Exercice, cle: &str) -> Option<&'a str> {
    ex.donnees.get(cle).and_then(Value::as_str)
}

fn nombres_anglais() -> impl Iterator<Item = &'static str> {
    vocabulaire::lexique()
        .iter()
        .filter(|p| p.famille == "nombres")
        .map(|p| p.anglais.as_str())
}

/// Pour « seven en français ? », la valeur 7 : l'écrire en chiffres donnerait la réponse.
fn valeur_nombre_anglais(ex: &Exercice) -> Option<u32> {
    if texte_de(ex, "famille") != Some("nombres") {
        return None;
    }
    let anglais = texte_de(ex, "anglais")?;
    nombres_anglais()
        .position(|mot| mot == anglais)
        .map(|rang| rang as u32)
}

fn sans_et(mots: &[String]) -> Vec<String> {
    mots.iter().filter(|m| *m != "et").cloned().collect()
}

/// Vrai si `texte` donne la réponse : en chiffres (virgule ou point, espaces de milliers),
/// en lettres pour un entier, ou le mot lui-même, en mot entier, sans accents ni casse.
pub fn contient_la_reponse(texte: &str, ex: &Exercice) -> bool {
    let mots_texte = mots(texte);
    if let Some(valeur) = valeur(&ex.reponse) {
        let en_chiffres = mots_texte
            .iter()
            .filter(|m| m.starts_with(|c: char| c.is_ascii_digit()))
            .any(|m| lire_nombre(m).is_some_and(|v| v == valeur));
        if en_chiffres {
            return true;
        }
        if valeur.is_integer() && valeur < entier(1_000_000) {
            let n = valeur.to_integer().to_u32().unwrap_or_default();
            return contient_suite(&sans_et(&mots_texte), &suite_lettres(n));
        }
        return false;
    }
    if contient_suite(&mots_texte, &mots(&ex.reponse)) {
        return true;
    }
    match valeur_nombre_anglais(ex) {
        Some(nombre) => {
            mots_texte.iter().any(|m| *m == nombre.to_string())
                || contient_suite(&sans_et(&mots_texte), &suite_lettres(nombre))
        }
        None => false,
    }
}

// --- Les paliers -----------------------------------------------------------------------

fn e(x: &Fraction) -> Result<String, Abandon> {
    Ok(ecrire_reponse(x)?)
}

fn partie_entiere(n: &Fraction) -> Fraction {
    n.floor()
}

fn nombre_de_chiffres(n: &Fraction) -> u32 {
    n.to_integer().to_string().len() as u32
}

fn dix_puissance(exposant: u32) -> Fraction {
    Fraction::from_integer(BigInt::from(10).pow(exposant))
}

/// 17 → (10, 7), 1200 → (1000, 200), 2,5 → (2, 0,5). `None` si rien à décomposer.
fn decomposer(n: &Fraction) -> Option<(Fraction, Fraction)> {
    let entiere = partie_entiere(n);
    if entiere != *n {
        return if entiere.is_zero() {
            None
        } else {
            Some((entiere.clone(), n - entiere))
        };
    }
    if entiere < entier(10) {
        return None;
    }
    let puissance = dix_puissance(nombre_de_chiffres(&entiere) - 1);
    let tete = (&entiere / &puissance).floor() * &puissance;
    if tete == entiere {
        None
    } else {
        Some((tete.clone(), entiere - tete))
    }
}

/// 40 → (4, "dizaines"), 3000 → (3, "milliers") ; `None` si ce n'est pas un nombre rond.
fn rond(n: &Fraction) -> Option<(BigInt, &'static str)> {
    if !n.is_integer() || *n < entier(10) {
        return None;
    }
    let valeur = n.to_integer();
    [(1000, "milliers"), (100, "centaines"), (10, "dizaines")]
        .into_iter()
        .map(|(puissance, nom)| (BigInt::from(puissance), nom))
        .find(|(puissance, _)| (&valeur % puissance).is_zero())
        .map(|(puissance, nom)| (&valeur / puissance, nom))
}

fn doubles_moities(ex: &Exercice) -> Paliers {
    let n = texte_de(ex, "nombre").and_then(valeur).ok_or(Abandon)?;
    let double = texte_de(ex, "famille") == Some("doubles");
    let (mot, mot_majuscule) = if double {
        ("le double", "Le double")
    } else {
        ("la moitié", "La moitié")
    };
    let deux = entier(2);
    let mut paliers = vec![if double {
        format!(
            "Le double de {}, c'est {} ajouté à lui-même.",
            e(&n)?,
            e(&n)?
        )
    } else {
        format!(
            "La moitié de {}, c'est le nombre qui, ajouté à lui-même, donne {}.",
            e(&n)?,
            e(&n)?
        )
    }];
    if let Some((a, b)) = decomposer(&n) {
        paliers.push(format!(
            "Découpe : {} = {} + {}. Cherche {mot} de {}, puis {mot} de {}.",
            e(&n)?,
            e(&a)?,
            e(&b)?,
            e(&a)?,
            e(&b)?
        ));
        let (fa, fb) = if double {
            (&a * &deux, &b * &deux)
        } else {
            (&a / &deux, &b / &deux)
        };
        // Une moitié qui ne s'écrit pas en deux décimales : ce palier-là seulement saute.
        let troisieme = || -> Result<String, Abandon> {
            Ok(format!(
                "{mot_majuscule} de {} = {} et {mot} de {} = {} : il reste à les ajouter.",
                e(&a)?,
                e(&fa)?,
                e(&b)?,
                e(&fb)?
            ))
        };
        if let Ok(palier) = troisieme() {
            paliers.push(palier);
        }
    } else if let Some((combien, nom)) = rond(&n) {
        paliers.push(format!(
            "Pense en {nom} : {}, c'est {combien} {nom}.",
            e(&n)?
        ));
        paliers.push(format!(
            "Cherche {mot} de {combien}, puis compte-le en {nom}."
        ));
    } else if double {
        paliers.push(format!(
            "Sur tes doigts : {}, et encore {} de plus.",
            e(&n)?,
            e(&n)?
        ));
    } else {
        paliers.push(format!(
            "Imagine {} jetons partagés équitablement entre toi et ton ami : combien chacun ?",
            e(&n)?
        ));
    }
    Ok(paliers)
}

/// 348 → [300, 40, 8] ; 3,25 → [3, 0,25]. Les morceaux nuls sont omis.
fn par_rangs(n: &Fraction) -> Vec<Fraction> {
    let entiere = partie_entiere(n);
    if entiere != *n {
        return [entiere.clone(), n - &entiere]
            .into_iter()
            .filter(|x| !x.is_zero())
            .collect();
    }
    let chiffres = entiere.to_integer().to_string();
    let total = chiffres.len() as u32;
    let rangs: Vec<Fraction> = chiffres
        .chars()
        .enumerate()
        .filter(|(_, c)| *c != '0')
        .filter_map(|(i, c)| {
            Some(entier(i64::from(c.to_digit(10)?)) * dix_puissance(total - i as u32 - 1))
        })
        .collect();
    if rangs.is_empty() {
        vec![Fraction::zero()]
    } else {
        rangs
    }
}

fn grand_et_petit<'a>(a: &'a Fraction, b: &'a Fraction) -> (&'a Fraction, &'a Fraction) {
    if a >= b { (a, b) } else { (b, a) }
}

fn addition(a: &Fraction, b: &Fraction) -> Paliers {
    let decimaux = !a.is_integer() || !b.is_integer();
    let mut paliers = vec![format!(
        "Pose l'addition en colonnes : {}",
        if decimaux {
            "virgule sous virgule."
        } else {
            "unités sous unités, dizaines sous dizaines."
        }
    )];
    let (ra, rb) = (par_rangs(a), par_rangs(b));
    if decimaux {
        let (ea, eb) = (partie_entiere(a), partie_entiere(b));
        let (da, db) = (a - &ea, b - &eb);
        paliers.push(format!(
            "Ajoute les parties entières ({} + {}), puis les parties décimales ({} + {}).",
            e(&ea)?,
            e(&eb)?,
            e(&da)?,
            e(&db)?
        ));
        paliers.push(format!(
            "{} + {} = {} et {} + {} = {} : il reste à les ajouter.",
            e(&ea)?,
            e(&eb)?,
            e(&(&ea + &eb))?,
            e(&da)?,
            e(&db)?,
            e(&(&da + &db))?
        ));
    } else if ra.len() > 1 || rb.len() > 1 {
        let (grand, petit) = grand_et_petit(a, b);
        let morceaux = par_rangs(petit);
        let etapes = morceaux
            .iter()
            .map(|m| Ok(format!("+ {}", e(m)?)))
            .collect::<Result<Vec<_>, Abandon>>()?
            .join(" puis ");
        paliers.push(format!(
            "Pars de {} et ajoute en plusieurs bonds : {etapes}.",
            e(grand)?
        ));
        if morceaux.len() > 1 {
            paliers.push(format!(
                "{} + {} = {} : il reste à ajouter {}.",
                e(grand)?,
                e(&morceaux[0])?,
                e(&(grand + &morceaux[0]))?,
                e(&(petit - &morceaux[0]))?
            ));
        }
    } else {
        let (grand, petit) = grand_et_petit(a, b);
        paliers.push(format!(
            "Pars de {} et avance de {} sur tes doigts.",
            e(grand)?,
            e(petit)?
        ));
    }
    Ok(paliers)
}

fn soustraction(a: &Fraction, b: &Fraction) -> Paliers {
    let mut paliers =
        vec!["Pose la soustraction en colonnes, le plus grand nombre en haut.".to_owned()];
    let morceaux = par_rangs(b);
    if morceaux.len() > 1 {
        let etapes = morceaux
            .iter()
            .map(|m| Ok(format!("− {}", e(m)?)))
            .collect::<Result<Vec<_>, Abandon>>()?
            .join(" puis ");
        paliers.push(format!(
            "Pars de {} et retire en plusieurs bonds : {etapes}.",
            e(a)?
        ));
        paliers.push(format!(
            "{} − {} = {} : il reste à retirer {}.",
            e(a)?,
            e(&morceaux[0])?,
            e(&(a - &morceaux[0]))?,
            e(&(b - &morceaux[0]))?
        ));
    } else if b.is_integer() && *b < entier(10) {
        paliers.push(format!(
            "Compte à rebours depuis {}, de {} pas.",
            e(a)?,
            e(b)?
        ));
    } else if b.is_integer() {
        let rang = match nombre_de_chiffres(b) - 1 {
            1 => Some("dizaines"),
            2 => Some("centaines"),
            3 => Some("milliers"),
            _ => None,
        };
        if let Some(rang) = rang {
            paliers.push(format!(
                "Retire {} : commence par le chiffre des {rang} de {}.",
                e(b)?,
                e(a)?
            ));
        }
    }
    Ok(paliers)
}

fn multiplication(a: &Fraction, b: &Fraction) -> Paliers {
    if !a.is_integer() || !b.is_integer() {
        return Ok(vec![
            "Calcule d'abord sans les virgules, puis remets la virgule : autant de \
             chiffres après elle qu'en tout dans les deux nombres."
                .to_owned(),
        ]);
    }
    let (grand, petit) = grand_et_petit(a, b);
    let dix = entier(10);
    let mut paliers = vec![if *petit <= dix {
        format!("C'est dans la table de {}.", e(petit)?)
    } else {
        "Pose la multiplication.".to_owned()
    }];
    if let Some((x, y)) = decomposer(grand) {
        paliers.push(format!(
            "Découpe : {} × {} = {} × {} + {} × {}.",
            e(grand)?,
            e(petit)?,
            e(&x)?,
            e(petit)?,
            e(&y)?,
            e(petit)?
        ));
        paliers.push(format!(
            "{} × {} = {} et {} × {} = {} : il reste à les ajouter.",
            e(&x)?,
            e(petit)?,
            e(&(&x * petit))?,
            e(&y)?,
            e(petit)?,
            e(&(&y * petit))?
        ));
    } else if *grand < dix && *grand > entier(5) {
        let complement = &dix - grand;
        paliers.push(format!(
            "{} × {}, c'est {} × 10 moins {} × {}.",
            e(petit)?,
            e(grand)?,
            e(petit)?,
            e(petit)?,
            e(&complement)?
        ));
        paliers.push(format!(
            "{} × 10 = {} et {} × {} = {} : il reste à soustraire.",
            e(petit)?,
            e(&(petit * &dix))?,
            e(petit)?,
            e(&complement)?,
            e(&(petit * &complement))?
        ));
    }
    Ok(paliers)
}

fn division(a: &Fraction, b: &Fraction) -> Paliers {
    let mut paliers = vec![format!(
        "Cherche combien de fois {} entre dans {}.",
        e(b)?,
        e(a)?
    )];
    if !a.is_integer() || !b.is_integer() || b.is_zero() {
        return Ok(paliers);
    }
    let (dividende, diviseur) = (a.to_integer(), b.to_integer());
    let multiples: Vec<String> = (1..=5)
        .map(|k| (&diviseur * BigInt::from(k)).to_string())
        .collect();
    paliers.push(format!(
        "Les multiples de {diviseur} : {}… Lequel s'approche ?",
        multiples.join(", ")
    ));
    let chiffres = dividende.to_string();
    for i in 1..chiffres.len() {
        let Ok(tete) = chiffres[..i].parse::<BigInt>() else {
            break;
        };
        if tete >= diviseur {
            let (q, r) = (&tete / &diviseur, &tete % &diviseur);
            paliers.push(format!(
                "Première étape : dans {tete}, {diviseur} va {q} fois, il reste {r}. \
                 Abaisse le chiffre suivant."
            ));
            break;
        }
    }
    Ok(paliers)
}

fn calcul(expression: &str) -> Paliers {
    let Some(m) = BINAIRE.captures(expression) else {
        return Ok(vec![
            "Commence par ce qui est entre parenthèses, puis les × et les ÷, et termine \
             par les + et les −."
                .to_owned(),
        ]);
    };
    let a = lire_nombre(&m[1]).ok_or(Abandon)?;
    let b = lire_nombre(&m[3]).ok_or(Abandon)?;
    match &m[2] {
        "+" => addition(&a, &b),
        "-" => soustraction(&a, &b),
        "×" => multiplication(&a, &b),
        _ => division(&a, &b),
    }
}

fn phrase_famille(famille: &str) -> &'static str {
    match famille {
        "jours" => "C'est un jour de la semaine.",
        "mois" => "C'est un mois de l'année.",
        "nombres" => "Ce mot désigne une quantité, écrite en lettres.",
        "couleurs" => "C'est une couleur.",
        "salutations" => "C'est une façon de saluer quelqu'un.",
        _ => "Pense au sens du mot.",
    }
}

static TABLE_REPERES: LazyLock<HashMap<String, HashMap<String, String>>> = LazyLock::new(|| {
    // Constante du binaire, relue par les tests.
    toml::from_str(REPERES).expect("repères embarqués")
});

/// Les repères relus par un parent : famille → mot anglais → repère.
pub fn reperes() -> &'static HashMap<String, HashMap<String, String>> {
    &TABLE_REPERES
}

fn rang(ex: &Exercice) -> Result<String, Abandon> {
    let famille = texte_de(ex, "famille").ok_or(Abandon)?;
    let anglais = texte_de(ex, "anglais").ok_or(Abandon)?;
    let rang = vocabulaire::lexique()
        .iter()
        .filter(|p| p.famille == famille)
        .position(|p| p.anglais == anglais)
        .ok_or(Abandon)?
        + 1;
    Ok(if rang == 1 {
        "1er".to_owned()
    } else {
        format!("{rang}e")
    })
}

fn majuscule_initiale(texte: &str) -> String {
    let mut lettres = texte.chars();
    match lettres.next() {
        Some(premiere) => premiere.to_uppercase().chain(lettres).collect(),
        None => String::new(),
    }
}

fn anglais(ex: &Exercice) -> Paliers {
    let famille = texte_de(ex, "famille").unwrap_or_default();
    let vers_francais = texte_de(ex, "sens") == Some(vocabulaire::Sens::VersFrancais.nom());
    let reponse = ex.reponse.as_str();
    let langue = if vers_francais {
        "français"
    } else {
        "anglais"
    };
    let lettres = reponse.chars().filter(|c| c.is_alphabetic()).count();
    let forme = if reponse.contains(' ') {
        format!(", en {} mots", mots_python(reponse).count())
    } else if reponse.contains('-') {
        ", avec un tiret".to_owned()
    } else {
        String::new()
    };
    let initiale = reponse.chars().next().ok_or(Abandon)?;
    let mut paliers = vec![
        phrase_famille(famille).to_owned(),
        format!("Le mot {langue} commence par « {initiale} » et a {lettres} lettres{forme}."),
    ];
    match famille {
        "jours" => paliers.push(format!(
            "C'est le {} jour de la semaine, en comptant à la française.",
            rang(ex)?
        )),
        "mois" => paliers.push(format!("C'est le {} mois de l'année.", rang(ex)?)),
        "nombres" => paliers
            .push("Compte en anglais : one, two, three… jusqu'à retrouver ce nombre.".to_owned()),
        "couleurs" | "salutations" => {
            let mot = texte_de(ex, "anglais").ok_or(Abandon)?;
            if let Some(repere) = reperes().get(famille).and_then(|table| table.get(mot))
                && !repere.is_empty()
            {
                paliers.push(format!("{}.", majuscule_initiale(repere)));
            }
        }
        _ => {}
    }
    if lettres >= 6 {
        let derniere = reponse.chars().next_back().ok_or(Abandon)?;
        paliers.push(format!(
            "Le mot {langue} commence par « {} » et finit par « {derniere} ».",
            debut(reponse, 2)
        ));
    }
    Ok(paliers)
}

/// `not valeur` de Python : absent, nul, zéro, vide ou faux.
fn faux(valeur: Option<&Value>) -> bool {
    match valeur {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => !b,
        Some(Value::Number(n)) => n.as_f64() == Some(0.0),
        Some(Value::String(s)) => s.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
    }
}

fn fraction_de(valeur: &Value) -> Result<Fraction, Abandon> {
    match valeur {
        Value::Number(n) => n.as_i64().map(entier).ok_or(Abandon),
        Value::String(s) => lire_nombre(strip(s)).ok_or(Abandon),
        _ => Err(Abandon),
    }
}

fn candidats(ex: &Exercice) -> Paliers {
    let d = &ex.donnees;
    if ex.competence == vocabulaire::COMPETENCE {
        return anglais(ex);
    }
    if matches!(texte_de(ex, "famille"), Some("doubles" | "moities")) && d.contains_key("nombre") {
        return doubles_moities(ex);
    }
    if let Some(expression) = d.get("expression") {
        return calcul(expression.as_str().ok_or(Abandon)?);
    }
    if let (Some(dividende), Some(diviseur)) = (d.get("dividende"), d.get("diviseur"))
        && faux(d.get("reste"))
    {
        return division(&fraction_de(dividende)?, &fraction_de(diviseur)?);
    }
    Ok(Vec::new())
}

/// Les paliers d'indices de `ex`, du plus vague au plus précis. Jamais vide.
pub fn indices(ex: &Exercice) -> Vec<String> {
    let paliers: Vec<String> = candidats(ex)
        .unwrap_or_default()
        .into_iter()
        .filter(|p| longueur(p) <= LONGUEUR_MAX && !contient_la_reponse(p, ex))
        .take(4)
        .collect();
    if paliers.is_empty() {
        vec![SECOURS.to_owned()]
    } else {
        paliers
    }
}
