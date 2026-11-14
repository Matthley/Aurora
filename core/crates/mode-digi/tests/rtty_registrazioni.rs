//! Misura del ricevitore RTTY su registrazioni vere senza testo di riferimento: per ogni coppia di toni (centro indicato) si decodifica
//! il file intero con le impostazioni di base del servizio e si contano indizi di buona copia:
//! - nominativi validi (parole a forma di nominativo ripetute almeno due volte);
//! - frazione di caratteri con probabilita' >= 0,9;
//! - spazzatura: caratteri al minuto nei tratti di 10 s senza nessuna parola valida;
//! - parole da contest ("5NN", "599", "TU", "TEST", numeri progressivi).
//!
//! I file si indicano con `AURORA_RTTY_WAV` (percorsi separati da `;`, ognuno seguito da `@` e dai centri separati da virgole, per
//! esempio `prova.wav@1976,2806`); senza la variabile si usano le due registrazioni del proprietario, se ci sono. `RTTY_SCAN` (qualsiasi
//! valore) stampa invece le coppie di toni piu' frequenti nel tempo, per scegliere i centri.
//!
//! `cargo test --release -j 2 -p mode-digi --test rtty_registrazioni -- --ignored --nocapture --test-threads=1`

use mode_digi::rtty::{RttyDemodulator, RttyParams};
use std::collections::HashMap;
use std::path::Path;

const RATE: f64 = 12_000.0;
/// Lunghezza dei tratti per la spazzatura (s).
const TRATTO_S: f64 = 10.0;

/// Le registrazioni di base: file e centri delle coppie (la seconda scelta dalla scansione `RTTY_SCAN`, anche deboli accanto a forti).
fn registrazioni() -> Vec<(String, Vec<f32>)> {
    if let Ok(v) = std::env::var("AURORA_RTTY_WAV") {
        return v
            .split(';')
            .filter_map(|s| {
                let (f, c) = s.split_once('@')?;
                Some((f.to_string(), c.split(',').filter_map(|x| x.trim().parse().ok()).collect()))
            })
            .collect();
    }
    let home = std::env::var("USERPROFILE").unwrap_or_default();
    vec![
        (format!("{home}\\Desktop\\tmp12856.wav"), vec![1976.0, 2806.0]),
        (format!("{home}\\Desktop\\tmp30515.wav"), CONTEST.to_vec()),
    ]
}

/// I centri scelti sulla registrazione del contest (scansione: i piu' presenti nel tempo, con coppie vicine come 1030/1070, 1470/1510 e
/// 2160/2190 dove una stazione debole sta accanto a una forte).
const CONTEST: [f32; 10] = [1030.0, 1070.0, 1290.0, 1470.0, 1510.0, 1760.0, 2090.0, 2160.0, 2190.0, 2520.0];

/// Legge un WAV PCM a 16 bit mono (canale sinistro se stereo) e lo porta a 12 kHz.
fn leggi(path: &Path) -> Option<Vec<f32>> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 44 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return None;
    }
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let (mut canali, mut rate, mut bits) = (1usize, 0u32, 0u16);
    let mut i = 12;
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let len = u32_at(i + 4) as usize;
        let body = i + 8;
        if id == b"fmt " {
            canali = usize::from(u16_at(body + 2)).max(1);
            rate = u32_at(body + 4);
            bits = u16_at(body + 14);
        } else if id == b"data" {
            if bits != 16 {
                return None;
            }
            let end = (body + len).min(b.len());
            let x: Vec<f32> = b[body..end].chunks_exact(2 * canali).map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0).collect();
            return Some(ricampiona(&x, f64::from(rate)));
        }
        i = body + len + (len & 1);
    }
    None
}

/// Ricampionamento lineare (le registrazioni sono a 11025 Hz: si sale di poco, e i segnali stanno sotto i 3 kHz).
fn ricampiona(x: &[f32], rate: f64) -> Vec<f32> {
    if (rate - RATE).abs() < 1.0 {
        return x.to_vec();
    }
    let n = (x.len() as f64 * RATE / rate) as usize;
    (0..n)
        .map(|k| {
            let t = k as f64 * rate / RATE;
            let i = t.floor() as usize;
            let f = (t - i as f64) as f32;
            let a = x.get(i).copied().unwrap_or(0.0);
            let b = x.get(i + 1).copied().unwrap_or(a);
            a + (b - a) * f
        })
        .collect()
}

/// Forma di nominativo: 1-2 caratteri di prefisso (lettere o cifra+lettera), una cifra, 1-4 lettere; opzionale "/P" ecc. tolto.
fn nominativo(w: &str) -> bool {
    let w = w.split('/').max_by_key(|p| p.len()).unwrap_or("");
    let b = w.as_bytes();
    if !(3..=7).contains(&b.len()) || !b.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return false;
    }
    // l'ultima cifra separa prefisso e suffisso
    let Some(d) = b.iter().rposition(|c| c.is_ascii_digit()) else { return false };
    let (pre, suf) = (&b[..d], &b[d + 1..]);
    (1..=4).contains(&suf.len())
        && suf.iter().all(|c| c.is_ascii_uppercase())
        && (1..=3).contains(&pre.len())
        && pre.iter().any(|c| c.is_ascii_uppercase())
}

fn contest(w: &str) -> bool {
    matches!(w, "5NN" | "599" | "TU" | "TEST") || ((3..=4).contains(&w.len()) && w.bytes().all(|c| c.is_ascii_digit() || c == b'N' || c == b'T') && w.bytes().any(|c| c.is_ascii_digit()))
}

#[derive(Default, Debug, Clone, Copy)]
struct Misura {
    nominativi: usize,
    sicuri: f64,
    spazzatura: f64,
    contest: usize,
    caratteri: usize,
}

fn misura(audio: &[f32], centre: f32) -> Misura {
    let mut d = RttyDemodulator::new(RttyParams { centre_hz: centre, shift_hz: 170.0, baud: 45.45, reverse: false });
    d.set_uos(true);
    d.set_afc(true);
    let tratto = (TRATTO_S * RATE) as usize;
    let (mut testo, mut tratti) = (String::new(), Vec::new());
    let (mut tot, mut sicuri) = (0usize, 0usize);
    for blocco in audio.chunks(tratto) {
        let mut s = String::new();
        for c in d.process_chars(blocco) {
            tot += 1;
            if c.prob >= 0.9 {
                sicuri += 1;
            }
            s.push(c.ch);
        }
        testo.push_str(&s);
        tratti.push(s);
    }
    let parole = |s: &str| -> Vec<String> { s.split(|c: char| !c.is_ascii_alphanumeric() && c != '/').filter(|w| !w.is_empty()).map(str::to_uppercase).collect() };
    let mut conta: HashMap<String, usize> = HashMap::new();
    let mut hits = 0;
    for w in parole(&testo) {
        if nominativo(&w) {
            *conta.entry(w.clone()).or_default() += 1;
        }
        if contest(&w) {
            hits += 1;
        }
    }
    let validi: std::collections::HashSet<&String> = conta.iter().filter(|(_, n)| **n >= 2).map(|(w, _)| w).collect();
    let (mut sporchi, mut min_sporchi) = (0usize, 0.0);
    for t in &tratti {
        let ok = parole(t).iter().any(|w| validi.contains(w) || contest(w));
        if !ok {
            sporchi += t.chars().filter(|c| !c.is_whitespace()).count();
            min_sporchi += TRATTO_S / 60.0;
        }
    }
    Misura {
        nominativi: validi.len(),
        sicuri: if tot > 0 { sicuri as f64 / tot as f64 } else { 0.0 },
        spazzatura: if min_sporchi > 0.0 { sporchi as f64 / min_sporchi } else { 0.0 },
        contest: hits,
        caratteri: tot,
    }
}

/// Scansione: per ogni minuto lo spettro medio, e le coppie (toni a ±85 Hz) che superano di 10 dB il rumore; si contano i minuti.
fn scansione(audio: &[f32]) {
    use rustfft::{FftPlanner, num_complex::Complex};
    const N: usize = 4096;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let bin = RATE / N as f64;
    let mut minuti: HashMap<i32, usize> = HashMap::new();
    for min in audio.chunks((60.0 * RATE) as usize) {
        let mut p = vec![0.0f32; N / 2];
        let mut k = 0;
        for w in min.chunks_exact(N) {
            let mut buf: Vec<Complex<f32>> = w
                .iter()
                .enumerate()
                .map(|(i, &x)| Complex::new(x * (0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()), 0.0))
                .collect();
            fft.process(&mut buf);
            for (j, c) in buf.iter().take(N / 2).enumerate() {
                p[j] += c.norm_sqr();
            }
            k += 1;
        }
        if k == 0 {
            continue;
        }
        let mut s = p.clone();
        s.sort_by(f32::total_cmp);
        let rumore = s[s.len() / 2].max(1e-12);
        let at = |hz: f64| p[(hz / bin).round() as usize];
        let mut c = 300.0;
        let mut trovati: Vec<(f64, f32)> = Vec::new();
        while c < 2900.0 {
            let v = at(c - 85.0).min(at(c + 85.0)) / rumore;
            if v > 10.0 {
                trovati.push((c, v));
            }
            c += bin;
        }
        // un solo centro per gruppo vicino (il piu' forte entro 40 Hz)
        let mut scelti: Vec<(f64, f32)> = Vec::new();
        for (c, v) in trovati {
            match scelti.last_mut() {
                Some(l) if c - l.0 < 40.0 => {
                    if v > l.1 {
                        *l = (c, v);
                    }
                }
                _ => scelti.push((c, v)),
            }
        }
        for (c, _) in scelti {
            *minuti.entry((c / 10.0).round() as i32 * 10).or_default() += 1;
        }
    }
    let mut v: Vec<_> = minuti.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (c, n) in v.iter().take(30) {
        println!("centro {c} Hz: {n} minuti");
    }
}

#[test]
#[ignore]
fn registrazioni_vere() {
    let mut somma = Misura::default();
    let mut n = 0;
    for (file, centri) in registrazioni() {
        let Some(audio) = leggi(Path::new(&file)) else {
            println!("{file}: non letto, salto");
            continue;
        };
        println!("{file}: {:.1} min", audio.len() as f64 / RATE / 60.0);
        if std::env::var_os("RTTY_SCAN").is_some() {
            scansione(&audio);
            continue;
        }
        for c in centri.into_iter().filter(|&c| c > 0.0) {
            let m = misura(&audio, c);
            println!(
                "  {c:6.0} Hz: nominativi {:3}  sicuri {:5.1} %  spazzatura {:6.1} car/min  contest {:4}  caratteri {}",
                m.nominativi,
                100.0 * m.sicuri,
                m.spazzatura,
                m.contest,
                m.caratteri
            );
            somma.nominativi += m.nominativi;
            somma.sicuri += m.sicuri;
            somma.spazzatura += m.spazzatura;
            somma.contest += m.contest;
            somma.caratteri += m.caratteri;
            n += 1;
        }
    }
    if n > 0 {
        println!(
            "RIEPILOGO coppie {n}: nominativi {}  sicuri {:.1} %  spazzatura {:.1} car/min  contest {}  caratteri {}",
            somma.nominativi,
            100.0 * somma.sicuri / n as f64,
            somma.spazzatura / n as f64,
            somma.contest,
            somma.caratteri
        );
    }
}

#[test]
fn forma_dei_nominativi() {
    for w in ["IU3ABC", "K1AR", "DL1ABC", "9A1A", "OH2BH", "W1AW"] {
        assert!(nominativo(w), "{w}");
    }
    for w in ["TEST", "599", "5NN", "ABCDEF", "123", "E3"] {
        assert!(!nominativo(w), "{w}");
    }
}
