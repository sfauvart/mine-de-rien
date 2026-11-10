//! Journal de l'agent : une ligne datée par message, sur la sortie d'erreur, visible dans
//! `docker compose logs agent`. Ni le texte du chat ni une photo n'y passent jamais.
//!
//! Les tests peuvent capturer ce qui est journalisé dans leur fil, pour vérifier justement
//! qu'un texte n'y figure pas.

use std::cell::RefCell;

use chrono::Local;

thread_local! {
    static CAPTURE: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

fn ecrire(niveau: &str, message: &str) {
    let capture = CAPTURE.with_borrow_mut(|capture| match capture {
        Some(lignes) => {
            lignes.push(format!("{niveau} {message}"));
            true
        }
        None => false,
    });
    if !capture {
        eprintln!("{} {message}", Local::now().format("%Y-%m-%d %H:%M:%S,%3f"));
    }
}

pub fn info(message: impl AsRef<str>) {
    ecrire("INFO", message.as_ref());
}

pub fn avertissement(message: impl AsRef<str>) {
    ecrire("WARNING", message.as_ref());
}

/// Commence à capturer les messages de ce fil au lieu de les écrire.
pub fn capturer() {
    CAPTURE.with_borrow_mut(|capture| *capture = Some(Vec::new()));
}

/// Rend les messages capturés depuis `capturer()` et remet l'écriture normale.
pub fn captures() -> Vec<String> {
    CAPTURE.with_borrow_mut(|capture| capture.take().unwrap_or_default())
}
