//! Hasard des tirages, isolé derrière un type à nous.
//!
//! Les tests l'initialisent par une graine pour être rejouables. Les suites ne sont pas
//! celles du `random.Random` de Python : rien n'en dépend, aucune graine n'est stockée.

use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

pub struct Alea(StdRng);

impl Alea {
    /// Générateur imprévisible, initialisé par le système.
    pub fn nouveau() -> Self {
        Self(rand::make_rng())
    }

    /// Générateur rejouable : la même graine donne la même suite.
    pub fn graine(graine: u64) -> Self {
        Self(StdRng::seed_from_u64(graine))
    }

    /// Entier entre `min` et `max`, bornes comprises (le `randint` de Python).
    pub fn entier(&mut self, min: i64, max: i64) -> i64 {
        self.0.random_range(min..=max)
    }

    /// Indice dans une liste de `n` éléments. `n` doit être positif.
    pub fn indice(&mut self, n: usize) -> usize {
        self.0.random_range(0..n)
    }

    /// Un élément au hasard ; `None` si la liste est vide.
    pub fn choisir<'a, T>(&mut self, liste: &'a [T]) -> Option<&'a T> {
        if liste.is_empty() {
            None
        } else {
            Some(&liste[self.indice(liste.len())])
        }
    }

    /// Un indice tiré selon des poids : deux fois plus de chances pour un poids double. Les
    /// poids négatifs, nuls ou non finis ne sortent jamais ; `None` s'il n'en reste aucun.
    pub fn pondere(&mut self, poids: &[f64]) -> Option<usize> {
        let utile = |p: f64| if p.is_finite() && p > 0.0 { p } else { 0.0 };
        let total: f64 = poids.iter().map(|&p| utile(p)).sum();
        if total <= 0.0 || !total.is_finite() {
            return None;
        }
        let mut reste = self.0.random_range(0.0..total);
        let mut dernier = None;
        for (i, &p) in poids.iter().enumerate() {
            let p = utile(p);
            if p <= 0.0 {
                continue;
            }
            if reste < p {
                return Some(i);
            }
            reste -= p;
            dernier = Some(i);
        }
        // Arrondis : la somme partielle peut manquer le total d'un cheveu.
        dernier
    }

    /// Mélange sur place (Fisher-Yates).
    pub fn melanger<T>(&mut self, liste: &mut [T]) {
        for i in (1..liste.len()).rev() {
            liste.swap(i, self.indice(i + 1));
        }
    }

    /// `n` éléments distincts au hasard, dans un ordre au hasard (le `sample` de Python).
    /// Rend toute la liste mélangée si elle a moins de `n` éléments.
    pub fn echantillon<T: Clone>(&mut self, liste: &[T], n: usize) -> Vec<T> {
        let mut copie = liste.to_vec();
        self.melanger(&mut copie);
        copie.truncate(n);
        copie
    }
}
