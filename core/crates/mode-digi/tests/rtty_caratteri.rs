//! Banco RTTY a livello di carattere (`docs/banco-rtty.md`): RTTY sintetico con testo noto (ITA2, 45,45 baud, 170 Hz, stop 1,5 bit,
//! 12 kHz) per il ricevitore a canale singolo (`rtty::RttyDemodulator`) e per quello di tutta la banda (`rtty_multi::RttyMulti`, il motore
//! di BigScan). Misura caratteri giusti, sbagliati, mancanti e inseriti, CER, nominativi, caratteri falsi nei silenzi, ritardo e tempo di
//! processore.
//!
//! `nice cargo test --release -p mode-digi -j 2 --test rtty_caratteri -- --ignored --nocapture --test-threads=1`
//!
//! Registrazione vera: `AURORA_RTTY_WAV` (mono, PCM 16 bit) con la trascrizione accanto (stesso nome, `.txt`) e `AURORA_RTTY_CENTRE`
//! (Hz, di base 1500); senza, il caso si salta. `AURORA_RTTY_SEME` cambia il seme, `AURORA_STAMPA` stampa testo atteso e letto.

mod banco_rtty;

use banco_rtty::*;
use mode_digi::rtty::{self, RttyDemodulator, RttyParams};
use mode_digi::rtty_multi::RttyMulti;
use mode_sstv::resample::Resampler;
use std::path::Path;
use std::time::Instant;

const CENTRO: f64 = 1500.0;
const SEME_BASE: u64 = 20261115;
/// Il seme del testo e del rumore: `AURORA_RTTY_SEME` per ripetere il banco su altri casi.
fn seme() -> u64 {
    std::env::var("AURORA_RTTY_SEME").ok().and_then(|v| v.parse().ok()).unwrap_or(SEME_BASE)
}
/// Blocco d'audio passato ai ricevitori (10 ms): il ritardo si misura a questa grana.
const BLOCCO: usize = 120;

fn singolo(centro: f64) -> RttyDemodulator {
    // come il servizio con AFC, UOS e squelch accesi
    let mut d = RttyDemodulator::new(RttyParams { centre_hz: centro as f32, shift_hz: SHIFT as f32, baud: BAUD as f32, reverse: false });
    d.set_afc(true);
    d.set_uos(true);
    d.set_squelch_prob(Some((rtty::SQUELCH_OPEN, rtty::SQUELCH_CLOSE)));
    d
}

/// Caratteri letti con l'istante (s) di fine del blocco in cui escono, e ms di processore per secondo d'audio.
fn passa_singolo(audio: &[f32], centro: f64) -> (Vec<(char, f64)>, f64) {
    let mut d = singolo(centro);
    let mut out = Vec::new();
    let t0 = Instant::now();
    let mut n = 0usize;
    for b in audio.chunks(BLOCCO) {
        n += b.len();
        let t = n as f64 / RATE;
        out.extend(d.process_chars(b).into_iter().map(|c| (c.ch, t)));
    }
    (out, cpu(t0, audio.len()))
}

/// Come [`passa_singolo`] con `RttyMulti`: i caratteri dei canali entro mezzo shift da `centro` (tutti, se `None`).
fn passa_multi(audio: &[f32], centro: Option<f64>) -> (Vec<(char, f64)>, f64) {
    let mut m = RttyMulti::new(SHIFT as f32, BAUD as f32);
    let mut out = Vec::new();
    let t0 = Instant::now();
    let mut n = 0usize;
    for b in audio.chunks(BLOCCO) {
        n += b.len();
        let t = n as f64 / RATE;
        for lt in m.process(b) {
            if centro.is_none_or(|c| (f64::from(lt.centre_hz) - c).abs() < SHIFT / 2.0) {
                out.extend(lt.chars.iter().map(|c| (c.ch, t)));
            }
        }
    }
    (out, cpu(t0, audio.len()))
}

fn cpu(t0: Instant, campioni: usize) -> f64 {
    t0.elapsed().as_secs_f64() * 1000.0 / (campioni as f64 / RATE)
}

struct Scenario {
    nome: &'static str,
    snr_db: f64,
    fading: Fading,
    deriva_hz: f64,
    /// Stazione vicina: distanza (Hz) e dB rispetto alla voluta.
    vicina: Option<(f64, f64)>,
    silenzio_s: f64,
}

fn sc(nome: &'static str, snr_db: f64) -> Scenario {
    Scenario { nome, snr_db, fading: Fading::Nessuno, deriva_hz: 0.0, vicina: None, silenzio_s: 1.0 }
}

fn scenari() -> Vec<Scenario> {
    vec![
        sc("AWGN +10 dB", 10.0),
        sc("AWGN 0 dB", 0.0),
        sc("AWGN -5 dB", -5.0),
        sc("AWGN -8 dB", -8.0),
        sc("AWGN -10 dB", -10.0),
        Scenario { fading: Fading::Piatto(0.2), ..sc("Rayleigh piatto 0 dB", 0.0) },
        Scenario { fading: Fading::Selettivo(0.5), ..sc("selettivo 0 dB", 0.0) },
        Scenario { vicina: Some((250.0, 20.0)), ..sc("vicina +20dB@250 0dB", 0.0) },
        Scenario { deriva_hz: 40.0, ..sc("deriva 40 Hz 0 dB", 0.0) },
        Scenario { silenzio_s: 5.0, ..sc("silenzi 5 s +3 dB", 3.0) },
    ]
}

fn genera(s: &Scenario, seme: u64) -> (Trama, Vec<f32>) {
    let mut r = Rng::new(seme);
    let tr = trama(&testo_contest(seme, 60.0, s.silenzio_s));
    let mut audio = sintetizza(&tr, CENTRO, s.deriva_hz, AMP, s.fading, &mut r);
    if let Some((df, db)) = s.vicina {
        let altra = trama(&testo_contest(seme ^ 0xABCD, 70.0, 0.3));
        let x = sintetizza(&altra, CENTRO + df, 0.0, AMP * 10f64.powf(db / 20.0), Fading::Nessuno, &mut r);
        for (a, b) in audio.iter_mut().zip(x) {
            *a += b;
        }
    }
    // un secondo di solo rumore in fondo: i ricevitori con piu' ritardo consegnano anche l'ultimo carattere, come farebbero sull'audio
    // che continua
    audio.extend(std::iter::repeat_n(0.0, RATE as usize));
    aggiungi_rumore(&mut audio, sigma_per_snr(s.snr_db, AMP), &mut r);
    (tr, audio)
}

#[test]
#[ignore]
fn banco_rtty_caratteri() {
    println!("\nBanco RTTY a livello di carattere (seme {}, S/N in 2,5 kHz, centro {CENTRO} Hz)", seme());
    println!("{}", intestazione());
    for s in scenari() {
        let (tr, audio) = genera(&s, seme());
        let (l1, c1) = passa_singolo(&audio, CENTRO);
        let mut m1 = misura(&tr.caratteri, &l1, &tr.silenzi, true);
        m1.cpu_ms_s = c1;
        println!("{}", riga(s.nome, "singolo", &m1));
        if std::env::var_os("AURORA_STAMPA").is_some() {
            println!("ATTESO {:?}\nLETTO  {:?}", tr.caratteri.iter().map(|c| c.0).collect::<String>(), l1.iter().map(|c| c.0).collect::<String>());
        }
        let (l2, c2) = passa_multi(&audio, Some(CENTRO));
        let mut m2 = misura(&tr.caratteri, &l2, &tr.silenzi, true);
        m2.cpu_ms_s = c2;
        println!("{}", riga(s.nome, "multi", &m2));
    }
    // solo rumore, 60 s: tutto e' silenzio
    let mut r = Rng::new(seme());
    let mut audio = vec![0f32; (60.0 * RATE) as usize];
    aggiungi_rumore(&mut audio, sigma_per_snr(0.0, AMP), &mut r);
    let silenzio = [(-1.0, 60.0)];
    let (l1, c1) = passa_singolo(&audio, CENTRO);
    let mut m1 = misura(&[], &l1, &silenzio, false);
    m1.cpu_ms_s = c1;
    println!("{}", riga("solo rumore 60 s", "singolo", &m1));
    let (l2, c2) = passa_multi(&audio, None);
    let mut m2 = misura(&[], &l2, &silenzio, false);
    m2.cpu_ms_s = c2;
    println!("{}", riga("solo rumore 60 s", "multi", &m2));
}

/// Trasmissione -> silenzio (2-5 s, solo rumore) -> trasmissione, ripetuto: pulito, evanescenza su un solo tono, crepitii di 5-50 ms.
#[test]
#[ignore]
fn banco_rtty_silenzi() {
    println!("\nBanco RTTY: trasmissioni separate da 2-5 s di solo rumore (seme {}, S/N in 2,5 kHz, centro {CENTRO} Hz)", seme());
    println!("{}", intestazione_silenzi());
    let varianti: [(&str, f64, Fading, bool); 6] = [
        ("pulito +3 dB", 3.0, Fading::Nessuno, false),
        ("pulito -5 dB", -5.0, Fading::Nessuno, false),
        ("un tono svanisce +3dB", 3.0, Fading::UnTono(0.5), false),
        ("un tono svanisce -3dB", -3.0, Fading::UnTono(0.5), false),
        ("crepitii +3 dB", 3.0, Fading::Nessuno, true),
        ("crepitii -3 dB", -3.0, Fading::Nessuno, true),
    ];
    for (nome, snr, fading, impulsi) in varianti {
        let mut r = Rng::new(seme());
        let tr = trama(&testo_contest_silenzi(seme(), 90.0, 2.0, 5.0));
        let mut audio = sintetizza(&tr, CENTRO, 0.0, AMP, fading, &mut r);
        audio.extend(std::iter::repeat_n(0.0, RATE as usize));
        aggiungi_rumore(&mut audio, sigma_per_snr(snr, AMP), &mut r);
        if impulsi {
            aggiungi_impulsi(&mut audio, 2.0, 4.0 * AMP, &mut r);
        }
        let (l1, _) = passa_singolo(&audio, CENTRO);
        let cer1 = misura(&tr.caratteri, &l1, &[], false).cer();
        if std::env::var_os("AURORA_STAMPA").is_some() {
            println!("{nome}\nATTESO {:?}\nLETTO  {:?}", tr.caratteri.iter().map(|c| c.0).collect::<String>(), l1.iter().map(|c| c.0).collect::<String>());
        }
        println!("{}", riga_silenzi(nome, "singolo", tr.passaggi.len(), cer1, &misura_silenzi(&tr, &l1)));
        let (l2, _) = passa_multi(&audio, Some(CENTRO));
        let cer2 = misura(&tr.caratteri, &l2, &[], false).cer();
        println!("{}", riga_silenzi(nome, "multi", tr.passaggi.len(), cer2, &misura_silenzi(&tr, &l2)));
    }
}

fn leggi_wav(path: &Path) -> Option<(u32, Vec<f32>)> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut canali, mut rate) = (1usize, 0u32);
    let mut i = 12;
    while i + 8 <= b.len() {
        let len = u32_at(i + 4) as usize;
        let body = i + 8;
        if &b[i..i + 4] == b"fmt " {
            canali = usize::from(u16_at(body + 2)).max(1);
            rate = u32_at(body + 4);
        } else if &b[i..i + 4] == b"data" {
            let end = (body + len).min(b.len());
            let x = b[body..end].chunks_exact(2 * canali).map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0).collect();
            return Some((rate, x));
        }
        i = body + len + (len & 1);
    }
    None
}

#[test]
#[ignore]
fn banco_rtty_registrazione() {
    let Some(wav) = std::env::var_os("AURORA_RTTY_WAV") else {
        println!("AURORA_RTTY_WAV non impostata: caso saltato");
        return;
    };
    let wav = Path::new(&wav);
    let rif = wav.with_extension("txt");
    let Ok(testo) = std::fs::read_to_string(&rif) else {
        println!("manca la trascrizione {}: caso saltato", rif.display());
        return;
    };
    let centro: f64 = std::env::var("AURORA_RTTY_CENTRE").ok().and_then(|v| v.parse().ok()).unwrap_or(1500.0);
    let (rate, x) = leggi_wav(wav).expect("WAV mono/stereo PCM 16 bit");
    let mut rs = Resampler::new(rate);
    let mut audio = Vec::new();
    for b in x.chunks((rate / 10).max(1) as usize) {
        rs.process(b, &mut audio);
    }
    let atteso: Vec<(char, f64)> = testo.chars().map(|c| (c, 0.0)).collect();
    println!("\nRegistrazione {} (centro {centro} Hz)", wav.display());
    println!("{}", intestazione());
    let (l1, c1) = passa_singolo(&audio, centro);
    let mut m1 = misura(&atteso, &l1, &[], false);
    m1.cpu_ms_s = c1;
    println!("{}", riga("registrazione", "singolo", &m1));
    let (l2, c2) = passa_multi(&audio, Some(centro));
    let mut m2 = misura(&atteso, &l2, &[], false);
    m2.cpu_ms_s = c2;
    println!("{}", riga("registrazione", "multi", &m2));
}

#[test]
fn generatore_baudot_rilegge_il_testo() {
    let p = vec![Passaggio { silenzio_prima_s: 0.5, testo: "CQ TEST IU3ABC IO91WM 599 014 73 TU".into() }];
    let tr = trama(&p);
    assert_eq!(rileggi(&tr).trim(), "CQ TEST IU3ABC IO91WM 599 014 73 TU");
    // FIGS prima di 599, di nuovo FIGS dopo lo spazio fra le cifre (registro non noto), LTRS prima di TU
    let mandati: String = tr.caratteri.iter().map(|c| c.0).collect();
    assert_eq!(mandati.trim(), "CQ TEST IU3ABC IO91WM 599 014 73 TU");
    // i tempi crescono e l'ultimo carattere finisce prima della fine del segnale
    assert!(tr.caratteri.windows(2).all(|w| w[0].1 < w[1].1));
    assert!(tr.caratteri.last().is_some_and(|c| c.1 < tr.durata_s));
    assert_eq!(tr.silenzi.len(), 1);
}

#[test]
fn allineamento_conta_le_operazioni() {
    let a: Vec<char> = "IU3ABC 599".chars().collect();
    let b: Vec<char> = "IU3AXC 5999".chars().collect();
    let ops = allinea(&a, &b);
    let sb = ops.iter().filter(|o| matches!(o, Op::Sbagliato(..))).count();
    let ins = ops.iter().filter(|o| matches!(o, Op::Inserito(_))).count();
    let manc = ops.iter().filter(|o| matches!(o, Op::Mancante(_))).count();
    assert_eq!((sb, ins, manc), (1, 1, 0));
    let t = |s: &str| s.chars().enumerate().map(|(i, c)| (c, i as f64)).collect::<Vec<_>>();
    let m = misura(&t("CQ IU3ABC DL1XYZ"), &t("CQ IU3ABC DL1XQZ  E"), &[], true);
    assert_eq!((m.call_tot, m.call_esatti, m.call_sbagliati), (2, 1, 1));
    assert_eq!((m.giusti, m.sbagliati, m.inseriti, m.mancanti), (15, 1, 2, 0));
}

#[test]
fn rumore_ha_il_rapporto_segnale_rumore_voluto() {
    let mut r = Rng::new(1);
    let mut x = vec![0f32; 120_000];
    let s = sigma_per_snr(0.0, AMP);
    aggiungi_rumore(&mut x, s, &mut r);
    let p: f64 = x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64;
    // a 0 dB in 2,5 kHz il rumore in tutta la banda (6 kHz) vale 2,4 volte la potenza del segnale
    let rapporto = p / (AMP * AMP / 2.0);
    assert!((rapporto - 2.4).abs() < 0.05, "{rapporto}");
}
