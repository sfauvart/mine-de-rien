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
