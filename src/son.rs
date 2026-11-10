//! Du WAV de la voix à l'Ogg Vorbis mono, seul format de son que lit Minecraft.
//!
//! La conversion se fait dans le binaire : l'image de l'agent ne contient que lui, donc ni
//! `ffmpeg` ni interpréteur de commandes. L'encodeur est celui de référence (libvorbis, en C,
//! embarqué par `vorbis_rs` comme SQLite l'est par `rusqlite`).
//!
//! Le WAV est lu à la main : PCM 16 bits, un ou deux canaux. Deux canaux sont mélangés en un
//! seul. Le numéro de flux Ogg est fixe : deux conversions du même WAV donnent le même fichier.

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

impl Pcm {
    pub fn duree_secondes(&self) -> f64 {
        self.echantillons.len() as f64 / f64::from(self.frequence)
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
