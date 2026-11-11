//! Du WAV de la voix à l'Ogg Vorbis mono que lit Minecraft.

use std::io::Cursor;

use mine_de_rien::son::{Pcm, encoder_ogg, lire_wav, wav_vers_ogg};
use vorbis_rs::VorbisDecoder;

/// Un WAV PCM 16 bits : une note de 440 Hz sur chaque canal.
fn wav(frequence: u32, canaux: u16, trames: usize) -> Vec<u8> {
    let mut donnees = Vec::new();
    for i in 0..trames {
        let t = i as f32 / frequence as f32;
        let valeur = ((t * 440.0 * std::f32::consts::TAU).sin() * 12_000.0) as i16;
        for _ in 0..canaux {
            donnees.extend_from_slice(&valeur.to_le_bytes());
        }
    }
    avec_entete(1, frequence, canaux, 16, &donnees)
}

fn avec_entete(codage: u16, frequence: u32, canaux: u16, bits: u16, donnees: &[u8]) -> Vec<u8> {
    let alignement = canaux * bits / 8;
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&codage.to_le_bytes());
    w.extend_from_slice(&canaux.to_le_bytes());
    w.extend_from_slice(&frequence.to_le_bytes());
    w.extend_from_slice(&(frequence * u32::from(alignement)).to_le_bytes());
    w.extend_from_slice(&alignement.to_le_bytes());
    w.extend_from_slice(&bits.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(donnees);
    w
}

/// Canaux, fréquence et nombre de trames d'un Ogg Vorbis, relu par le décodeur de référence.
fn relire(ogg: &[u8]) -> (u8, u32, usize) {
    let mut decodeur = VorbisDecoder::new(Cursor::new(ogg.to_vec())).unwrap();
    let canaux = decodeur.channels().get();
    let frequence = decodeur.sampling_frequency().get();
    let mut trames = 0;
    while let Some(bloc) = decodeur.decode_audio_block().unwrap() {
        trames += bloc.samples()[0].len();
    }
    (canaux, frequence, trames)
}

#[test]
fn wav_mono_devient_un_ogg_vorbis_mono() {
    let ogg = wav_vers_ogg(&wav(22_050, 1, 22_050)).unwrap();
    assert!(ogg.starts_with(b"OggS"));
    // En-tête d'identification Vorbis : « \x01vorbis », version, puis le nombre de canaux.
    let entete = ogg.windows(7).position(|f| f == b"\x01vorbis").unwrap();
    assert_eq!(ogg[entete + 11], 1);
    assert_eq!(
        u32::from_le_bytes(ogg[entete + 12..entete + 16].try_into().unwrap()),
        22_050
    );
    assert_eq!(relire(&ogg), (1, 22_050, 22_050));
}

#[test]
fn deux_canaux_sont_melanges_en_un_seul() {
    let pcm = lire_wav(&wav(44_100, 2, 4_410)).unwrap();
    assert_eq!(pcm.frequence, 44_100);
    assert_eq!(pcm.echantillons.len(), 4_410);
    assert!((pcm.duree_secondes() - 0.1).abs() < 1e-9);
    let (canaux, frequence, trames) = relire(&encoder_ogg(&pcm).unwrap());
    assert_eq!((canaux, frequence, trames), (1, 44_100, 4_410));
}

#[test]
fn echantillons_entre_moins_un_et_un() {
    let donnees: Vec<u8> = [i16::MIN, 0, i16::MAX]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let pcm = lire_wav(&avec_entete(1, 22_050, 1, 16, &donnees)).unwrap();
    assert_eq!(pcm.echantillons[0], -1.0);
    assert_eq!(pcm.echantillons[1], 0.0);
    assert!(pcm.echantillons[2] > 0.999 && pcm.echantillons[2] < 1.0);
}

#[test]
fn meme_wav_meme_ogg() {
    let source = wav(22_050, 1, 5_000);
    assert_eq!(
        wav_vers_ogg(&source).unwrap(),
        wav_vers_ogg(&source).unwrap()
    );
}

#[test]
fn bloc_inconnu_avant_les_donnees_est_saute() {
    let source = wav(22_050, 1, 1_000);
    // Un bloc « LIST » de taille impaire, donc suivi d'un octet d'alignement.
    let mut avec_liste = source[..12].to_vec();
    avec_liste.extend_from_slice(b"LIST");
    avec_liste.extend_from_slice(&3u32.to_le_bytes());
    avec_liste.extend_from_slice(b"abc\0");
    avec_liste.extend_from_slice(&source[12..]);
    assert_eq!(lire_wav(&avec_liste).unwrap(), lire_wav(&source).unwrap());
}

#[test]
fn taille_annoncee_trop_grande_s_arrete_a_la_fin_du_fichier() {
    let mut source = wav(22_050, 1, 1_000);
    let position = source.windows(4).position(|f| f == b"data").unwrap() + 4;
    source[position..position + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(lire_wav(&source).unwrap().echantillons.len(), 1_000);
}

#[test]
fn wav_mal_forme_est_une_erreur_pas_une_panique() {
    let bon = wav(22_050, 1, 100);
    let refuses: Vec<(&str, Vec<u8>)> = vec![
        ("vide", Vec::new()),
        (
            "pas un WAV",
            b"OggS et la suite, qui n'est pas un WAV".to_vec(),
        ),
        ("en-tête seul", bon[..12].to_vec()),
        ("format tronqué", bon[..30].to_vec()),
        ("sans données", bon[..36].to_vec()),
        ("aucun échantillon", avec_entete(1, 22_050, 1, 16, &[])),
        ("8 bits", avec_entete(1, 22_050, 1, 8, &[0; 64])),
        ("flottant", avec_entete(3, 22_050, 1, 16, &[0; 64])),
        ("six canaux", avec_entete(1, 22_050, 6, 16, &[0; 120])),
        ("fréquence nulle", avec_entete(1, 0, 1, 16, &[0; 64])),
        ("données avant le format", {
            let mut w = bon[..12].to_vec();
            w.extend_from_slice(b"data");
            w.extend_from_slice(&4u32.to_le_bytes());
            w.extend_from_slice(&[0; 4]);
            w
        }),
    ];
    for (nom, octets) in refuses {
        let erreur = wav_vers_ogg(&octets).expect_err(nom);
        assert!(erreur.0.starts_with("Son illisible"), "{nom} : {erreur}");
    }
    // Toutes les coupes d'un fichier valide : une erreur ou un son plus court, jamais de panique.
    for fin in 0..bon.len() {
        let _ = lire_wav(&bon[..fin]);
    }
}

#[test]
fn son_sans_echantillon_ne_s_encode_pas_en_panique() {
    let vide = Pcm {
        frequence: 22_050,
        echantillons: Vec::new(),
    };
    // Un fichier Ogg sans son, ou une erreur : les deux sont acceptables, pas la panique.
    let _ = encoder_ogg(&vide);
    let sans_frequence = Pcm {
        frequence: 0,
        echantillons: vec![0.0; 10],
    };
    assert!(encoder_ogg(&sans_frequence).is_err());
}

/// Une note pure, d'amplitude donnée.
fn note(frequence: u32, hertz: f32, amplitude: f32, echantillons: usize) -> Pcm {
    Pcm {
        frequence,
        echantillons: (0..echantillons)
            .map(|i| {
                (i as f32 / frequence as f32 * hertz * std::f32::consts::TAU).sin() * amplitude
            })
            .collect(),
    }
}

#[test]
fn reechantillonner_garde_la_duree_a_un_echantillon_pres() {
    for (de, vers) in [
        (22_050, 48_000),
        (16_000, 48_000),
        (44_100, 48_000),
        (48_000, 22_050),
    ] {
        for longueur in [1usize, 2, 147, 1_000, 22_050, 30_001] {
            let source = note(de, 440.0, 0.5, longueur);
            let sortie = source.reechantillonner(vers);
            assert_eq!(sortie.frequence, vers);
            let attendu = longueur as f64 * f64::from(vers) / f64::from(de);
            assert!(
                (sortie.echantillons.len() as f64 - attendu).abs() <= 1.0,
                "{de} → {vers}, {longueur} échantillons : {}",
                sortie.echantillons.len()
            );
            assert!((sortie.duree_secondes() - source.duree_secondes()).abs() <= 1.0 / 22_050.0);
        }
    }
}

#[test]
fn reechantillonner_ne_sature_pas() {
    // Le pire cas : un signal carré à pleine échelle, dont l'interpolation dépasse.
    let carre = Pcm {
        frequence: 22_050,
        echantillons: (0..4_000)
            .map(|i| if (i / 25) % 2 == 0 { 1.0 } else { -1.0 })
            .collect(),
    };
    let sortie = carre.reechantillonner(48_000);
    assert!(sortie.echantillons.iter().all(|e| (-1.0..=1.0).contains(e)));
    // Et les octets déposés ne débordent pas d'un entier de 16 bits : pas de repli de signe.
    let octets = sortie.en_pcm16();
    assert_eq!(octets.len(), 2 * sortie.echantillons.len());
    for (e, o) in sortie.echantillons.iter().zip(octets.chunks_exact(2)) {
        let valeur = i16::from_le_bytes([o[0], o[1]]);
        assert!((f32::from(valeur) / 32_767.0 - e).abs() < 1e-4);
    }
}

#[test]
fn reechantillonner_garde_la_note_et_son_volume() {
    // Une note de 440 Hz d'amplitude 0,5 reste une note de 440 Hz d'amplitude 0,5.
    let sortie = note(22_050, 440.0, 0.5, 22_050).reechantillonner(48_000);
    let attendu = note(48_000, 440.0, 0.5, 48_000);
    // Hors des bords, où le filtre n'a pas tous ses voisins.
    for i in 200..47_800 {
        assert!(
            (sortie.echantillons[i] - attendu.echantillons[i]).abs() < 0.005,
            "échantillon {i} : {} au lieu de {}",
            sortie.echantillons[i],
            attendu.echantillons[i]
        );
    }
    // Un aigu proche de la limite de la voix (10 kHz) passe sans s'éteindre.
    let aigu = note(22_050, 10_000.0, 0.5, 22_050).reechantillonner(48_000);
    let crete = aigu.echantillons[1_000..47_000]
        .iter()
        .fold(0.0_f32, |m, e| m.max(e.abs()));
    assert!((0.45..=0.52).contains(&crete), "crête à {crete}");
}

#[test]
fn reechantillonner_a_la_meme_frequence_ne_change_rien() {
    let source = note(48_000, 440.0, 0.5, 960);
    assert_eq!(source.reechantillonner(48_000), source);
    let vide = Pcm {
        frequence: 22_050,
        echantillons: Vec::new(),
    };
    assert!(vide.reechantillonner(48_000).echantillons.is_empty());
}

#[test]
fn pcm16_est_petit_boutiste_et_borne() {
    let pcm = Pcm {
        frequence: 48_000,
        echantillons: vec![0.0, 1.0, -1.0, 0.5, 3.0, -3.0],
    };
    let valeurs: Vec<i16> = pcm
        .en_pcm16()
        .chunks_exact(2)
        .map(|o| i16::from_le_bytes([o[0], o[1]]))
        .collect();
    assert_eq!(valeurs, [0, 32_767, -32_767, 16_384, 32_767, -32_768]);
}
