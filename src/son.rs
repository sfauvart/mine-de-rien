//! Du WAV de la voix à l'Ogg Vorbis mono, seul format de son que lit Minecraft, et au PCM
//! brut à 48 000 Hz qu'attend le chat vocal.
//!
//! La conversion se fait dans le binaire : l'image de l'agent ne contient que lui, donc ni
//! `ffmpeg` ni interpréteur de commandes. L'encodeur est celui de référence (libvorbis, en C,
//! embarqué par `vorbis_rs` comme SQLite l'est par `rusqlite`).
//!
//! Le WAV est lu à la main : PCM 16 bits, un ou deux canaux. Deux canaux sont mélangés en un
//! seul. Le numéro de flux Ogg est fixe : deux conversions du même WAV donnent le même fichier.
//!
//! Le rééchantillonnage est écrit ici, sans crate ni outil externe : une interpolation par
//! sinus cardinal fenêtré, qui ne laisse pas passer d'aigus fantômes comme le ferait une
//! interpolation linéaire.

use std::num::{NonZeroU8, NonZeroU32};

use vorbis_rs::VorbisEncoderBuilder;

/// Le son n'a pas pu être converti ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurSon(pub String);

/// Un son décodé : des échantillons entre -1 et 1, sur un seul canal.
#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub frequence: u32,
    pub echantillons: Vec<f32>,
}

/// La fréquence du chat vocal (Simple Voice Chat), la seule qu'il sache jouer.
pub const FREQUENCE_DU_CHAT_VOCAL: u32 = 48_000;

/// Demi-largeur du filtre de rééchantillonnage, en échantillons de la fréquence la plus basse.
const DEMI_LARGEUR: f64 = 16.0;

impl Pcm {
    pub fn duree_secondes(&self) -> f64 {
        self.echantillons.len() as f64 / f64::from(self.frequence)
    }

    /// Le même son à une autre fréquence d'échantillonnage : même durée, à un échantillon
    /// près. Chaque échantillon de sortie est une moyenne de ses voisins d'entrée, pondérée
    /// par un sinus cardinal sous une fenêtre de Hann ; vers une fréquence plus basse, le
    /// filtre s'élargit pour couper ce qu'elle ne peut pas porter. La sortie reste entre -1
    /// et 1.
    pub fn reechantillonner(&self, frequence: u32) -> Pcm {
        if frequence == self.frequence || frequence == 0 || self.frequence == 0 {
            return self.clone();
        }
        let entree = &self.echantillons;
        let (de, vers) = (f64::from(self.frequence), f64::from(frequence));
        let longueur = (entree.len() as f64 * vers / de).round() as usize;
        // Part de la bande d'entrée que la sortie peut porter.
        let coupure = (vers / de).min(1.0);
        let largeur = DEMI_LARGEUR / coupure;
        let portee = largeur.ceil() as i64;
        let echantillons = (0..longueur)
            .map(|n| {
                let instant = n as f64 * de / vers;
                let centre = instant.floor() as i64;
                // `poids` compte aussi les voisins hors du son, qui valent zéro : le volume
                // ne gonfle pas au début ni à la fin.
                let (mut somme, mut poids) = (0.0_f64, 0.0_f64);
                for k in (centre - portee + 1)..=(centre + portee) {
                    let ecart = instant - k as f64;
                    if ecart.abs() >= largeur {
                        continue;
                    }
                    let fenetre = 0.5 * (1.0 + (std::f64::consts::PI * ecart / largeur).cos());
                    let coefficient = sinus_cardinal(coupure * ecart) * fenetre;
                    poids += coefficient;
                    if let Some(valeur) = usize::try_from(k).ok().and_then(|k| entree.get(k)) {
                        somme += f64::from(*valeur) * coefficient;
                    }
                }
                if poids.abs() < f64::EPSILON {
                    0.0
                } else {
                    (somme / poids).clamp(-1.0, 1.0) as f32
                }
            })
            .collect();
        Pcm {
            frequence,
            echantillons,
        }
    }

    /// Les échantillons en PCM brut, 16 bits petit-boutiste, sans en-tête : le format des
    /// sons déposés pour le chat vocal.
    pub fn en_pcm16(&self) -> Vec<u8> {
        self.echantillons
            .iter()
            .flat_map(|e| {
                let valeur = (f64::from(*e) * 32_767.0)
                    .round()
                    .clamp(-32_768.0, 32_767.0);
                (valeur as i16).to_le_bytes()
            })
            .collect()
    }
}

fn sinus_cardinal(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let angle = std::f64::consts::PI * x;
        angle.sin() / angle
    }
}

const NUMERO_DE_FLUX: i32 = 1;
const BLOC: usize = 4096;

fn illisible(raison: &str) -> ErreurSon {
    ErreurSon(format!("Son illisible : {raison}."))
}

fn u16_le(octets: &[u8], position: usize) -> Option<u16> {
    let o = octets.get(position..position + 2)?;
    Some(u16::from_le_bytes([o[0], o[1]]))
}

fn u32_le(octets: &[u8], position: usize) -> Option<u32> {
    let o = octets.get(position..position + 4)?;
    Some(u32::from_le_bytes([o[0], o[1], o[2], o[3]]))
}

/// Lit un fichier WAV PCM 16 bits, à un ou deux canaux.
pub fn lire_wav(wav: &[u8]) -> Result<Pcm, ErreurSon> {
    if wav.len() < 12 || &wav[..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err(illisible("ce n'est pas un fichier WAV"));
    }
    let mut format: Option<(u16, u32)> = None; // canaux, fréquence
    let mut position = 12;
    while let (Some(nom), Some(taille)) =
        (wav.get(position..position + 4), u32_le(wav, position + 4))
    {
        let debut = position + 8;
        // Un WAV écrit au fil de l'eau annonce parfois une taille fausse : on s'arrête à la fin.
        let fin = debut.saturating_add(taille as usize).min(wav.len());
        match nom {
            b"fmt " => {
                let (Some(codage), Some(canaux), Some(frequence), Some(bits)) = (
                    u16_le(wav, debut),
                    u16_le(wav, debut + 2),
                    u32_le(wav, debut + 4),
                    u16_le(wav, debut + 14),
                ) else {
                    return Err(illisible("en-tête de format tronqué"));
                };
                if codage != 1 || bits != 16 {
                    return Err(illisible("seul le PCM 16 bits est pris en charge"));
                }
                if !(1..=2).contains(&canaux) {
                    return Err(illisible("un ou deux canaux attendus"));
                }
                if !(8_000..=48_000).contains(&frequence) {
                    return Err(illisible("fréquence d'échantillonnage inattendue"));
                }
                format = Some((canaux, frequence));
            }
            b"data" => {
                let Some((canaux, frequence)) = format else {
                    return Err(illisible("les données précèdent le format"));
                };
                let canaux = usize::from(canaux);
                let echantillons: Vec<f32> = wav[debut..fin]
                    .chunks_exact(2 * canaux)
                    .map(|trame| {
                        let somme: f32 = trame
                            .chunks_exact(2)
                            .map(|o| f32::from(i16::from_le_bytes([o[0], o[1]])) / 32_768.0)
                            .sum();
                        somme / canaux as f32
                    })
                    .collect();
                if echantillons.is_empty() {
                    return Err(illisible("aucun échantillon"));
                }
                return Ok(Pcm {
                    frequence,
                    echantillons,
                });
            }
            _ => {}
        }
        // Les blocs sont alignés sur deux octets.
        position = fin + (taille as usize & 1);
    }
    Err(illisible("aucune donnée audio"))
}

/// Encode un son mono en Ogg Vorbis.
pub fn encoder_ogg(pcm: &Pcm) -> Result<Vec<u8>, ErreurSon> {
    let echec = |e: vorbis_rs::VorbisError| ErreurSon(format!("Encodage Vorbis impossible : {e}."));
    let frequence = NonZeroU32::new(pcm.frequence)
        .ok_or_else(|| illisible("fréquence d'échantillonnage nulle"))?;
    let mut encodeur = VorbisEncoderBuilder::new_with_serial(
        frequence,
        NonZeroU8::MIN,
        Vec::new(),
        NUMERO_DE_FLUX,
    )
    .build()
    .map_err(echec)?;
    for bloc in pcm.echantillons.chunks(BLOC) {
        encodeur.encode_audio_block([bloc]).map_err(echec)?;
    }
    encodeur.finish().map_err(echec)
}

/// Le WAV rendu par la voix devient l'Ogg Vorbis mono que le jeu sait lire.
pub fn wav_vers_ogg(wav: &[u8]) -> Result<Vec<u8>, ErreurSon> {
    encoder_ogg(&lire_wav(wav)?)
}
