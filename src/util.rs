//! Petits outils partagés, sans notion métier.

/// Écrit une chaîne comme le `repr()` de Python : les messages d'erreur citent ainsi la
/// valeur refusée, et ils sont comparés à ceux de la version Python.
pub(crate) fn repr(texte: &str) -> String {
    let guillemet = if texte.contains('\'') && !texte.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut sortie = String::with_capacity(texte.len() + 2);
    sortie.push(guillemet);
    for c in texte.chars() {
        match c {
            '\\' => sortie.push_str("\\\\"),
            '\n' => sortie.push_str("\\n"),
            '\r' => sortie.push_str("\\r"),
            '\t' => sortie.push_str("\\t"),
            c if c == guillemet => {
                sortie.push('\\');
                sortie.push(c);
            }
            c => sortie.push(c),
        }
    }
    sortie.push(guillemet);
    sortie
}

/// Espace au sens de `str.isspace()` de Python : celui de Rust, plus les séparateurs
/// U+001C à U+001F. `strip()` et `split()` du Python s'appuyaient dessus.
pub(crate) fn est_espace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()` de Python.
pub(crate) fn strip(texte: &str) -> &str {
    texte.trim_matches(est_espace)
}

/// `str.split()` de Python, sans argument : les mots, sans chaîne vide.
pub(crate) fn mots(texte: &str) -> impl Iterator<Item = &str> {
    texte.split(est_espace).filter(|mot| !mot.is_empty())
}

/// Longueur en caractères, celle de `len()` en Python. Jamais `str::len`, qui compte des octets.
pub(crate) fn longueur(texte: &str) -> usize {
    texte.chars().count()
}

/// Les `n` premiers caractères, comme `texte[:n]` en Python.
pub(crate) fn debut(texte: &str, n: usize) -> &str {
    match texte.char_indices().nth(n) {
        Some((octet, _)) => &texte[..octet],
        None => texte,
    }
}

fn ecrire_chaine_json(texte: &str, sortie: &mut String) {
    sortie.push('"');
    for c in texte.chars() {
        match c {
            '"' => sortie.push_str("\\\""),
            '\\' => sortie.push_str("\\\\"),
            '\n' => sortie.push_str("\\n"),
            '\r' => sortie.push_str("\\r"),
            '\t' => sortie.push_str("\\t"),
            '\u{8}' => sortie.push_str("\\b"),
            '\u{c}' => sortie.push_str("\\f"),
            c if (c as u32) < 0x20 => sortie.push_str(&format!("\\u{:04x}", c as u32)),
            c => sortie.push(c),
        }
    }
    sortie.push('"');
}

fn ecrire_json(valeur: &serde_json::Value, sortie: &mut String) {
    use serde_json::Value;
    match valeur {
        Value::String(texte) => ecrire_chaine_json(texte, sortie),
        Value::Array(liste) => {
            sortie.push('[');
            for (i, element) in liste.iter().enumerate() {
                if i > 0 {
                    sortie.push_str(", ");
                }
                ecrire_json(element, sortie);
            }
            sortie.push(']');
        }
        Value::Object(objet) => {
            sortie.push('{');
            for (i, (cle, element)) in objet.iter().enumerate() {
                if i > 0 {
                    sortie.push_str(", ");
                }
                ecrire_chaine_json(cle, sortie);
                sortie.push_str(": ");
                ecrire_json(element, sortie);
            }
            sortie.push('}');
        }
        autre => sortie.push_str(&autre.to_string()),
    }
}

/// `json.dumps(valeur, ensure_ascii=False)` de Python : séparateurs `", "` et `": "`,
/// accents en clair, clés dans l'ordre d'insertion. Les commandes Minecraft et la colonne
/// `donnees` de la base ont été écrites ainsi : on garde l'écriture au caractère près.
pub(crate) fn json_python(valeur: &serde_json::Value) -> String {
    let mut sortie = String::new();
    ecrire_json(valeur, &mut sortie);
    sortie
}
