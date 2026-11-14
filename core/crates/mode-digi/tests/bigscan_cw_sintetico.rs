//! Banco di prova sintetico di «tutta la banda» in CW: una banda da contest generata qui, con la verità nota, al posto di una
//! registrazione vera (che non c'e').
//!
//! Ogni banda ha `CW_BANCO_RUNNER` stazioni che chiamano CQ (di base 16) e le stazioni che rispondono loro, sulla stessa frequenza o
//! quasi: «CQ TEST IU3ABC IU3ABC», «K1XYZ», «K1XYZ 5NN 001», «TU 5NN 012», «TU IU3ABC TEST». Velocita' da 15 a 40 WPM (un terzo
//! delle stazioni oltre 33), S/N da -12 a +20 dB in 2,5 kHz, QSB, piccola deriva del tono, irregolarita' della manipolazione, stazioni a
//! 30-80 Hz l'una dall'altra. L'audio passa al `CwBandDecoder` a blocchi di 100 ms come nel servizio; ogni 5 s si fotografa l'elenco
//! che mostrerebbe l'interfaccia, con le stesse regole di `band.rs` (copiate da `bigscan_misura.rs`).
//!
//! Misura per ogni stazione che chiama CQ: se compare nell'elenco, dopo quanto dalla fine del primo CQ, la presenza mentre e' attiva e le
//! interruzioni; poi i nominativi falsi (nell'elenco come CQ ma di nessuno che chiama CQ). Stampa una riga RIEPILOGO e i trovati per
//! fascia di velocita'.
//! - `CW_BANCO_SEMI`: quante bande diverse (di base 2); `CW_BANCO_DURATA`: secondi d'audio per banda (di base 240);
//! - `CW_BANCO_SEME`: il primo seme (di base 1000);
//! - `CW_DEBUG=1`: stampa anche i canali alla fine di ogni banda.
//!
//! `CARGO_BUILD_JOBS=2 nice cargo test --release -j 2 -p mode-digi --test bigscan_cw_sintetico -- --ignored --nocapture --test-threads=1`

mod common;

use common::Rng;
use mode_digi::skim::calls::Role;
use mode_digi::skim::{BandDecoder, CwBandDecoder};
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

const RATE: f64 = 12_000.0;
const SNAP_S: f64 = 5.0;
/// Come `band.rs`.
const CALL_STALE_S: f64 = 75.0;
const CALL_STALE_CQ_S: f64 = 180.0;
const SUPERSEDED_S: f64 = 10.0;
const SUPERSEDED_GONE_S: f64 = 60.0;
const CALL_GONE_S: f64 = 180.0;
/// Deviazione del rumore bianco a 12 kHz (le ampiezze dei segnali seguono il loro S/N in 2,5 kHz).
const SIGMA: f64 = 0.02;

fn morse(c: u8) -> Option<&'static str> {
    const T: [(u8, &str); 37] = [
        (b'A', ".-"),
        (b'B', "-..."),
        (b'C', "-.-."),
        (b'D', "-.."),
        (b'E', "."),
        (b'F', "..-."),
        (b'G', "--."),
        (b'H', "...."),
        (b'I', ".."),
        (b'J', ".---"),
        (b'K', "-.-"),
        (b'L', ".-.."),
        (b'M', "--"),
        (b'N', "-."),
        (b'O', "---"),
        (b'P', ".--."),
        (b'Q', "--.-"),
        (b'R', ".-."),
        (b'S', "..."),
        (b'T', "-"),
        (b'U', "..-"),
        (b'V', "...-"),
        (b'W', ".--"),
        (b'X', "-..-"),
        (b'Y', "-.--"),
        (b'Z', "--.."),
        (b'0', "-----"),
        (b'1', ".----"),
        (b'2', "..---"),
        (b'3', "...--"),
        (b'4', "....-"),
        (b'5', "....."),
        (b'6', "-...."),
        (b'7', "--..."),
        (b'8', "---.."),
        (b'9', "----."),
        (b'/', "-..-."),
    ];
    T.iter().find(|(ch, _)| *ch == c).map(|(_, p)| *p)
}

/// Una stazione: tono, velocita', ampiezza, manipolazione, QSB e deriva.
#[derive(Clone)]
struct Stazione {
    call: String,
    hz: f64,
    wpm: f64,
    snr_db: f64,
    /// Irregolarita' relativa di ogni elemento (0,02 keyer, fino a 0,12 tasto semiautomatico).
    jitter: f64,
    /// Rapporto linea/punto.
    dash: f64,
    /// QSB: profondita' (dB) e periodo (s); fase.
    qsb_db: f64,
    qsb_s: f64,
    qsb_ph: f64,
    /// Deriva del tono (Hz/s) e oscillazione lenta (Hz).
    drift: f64,
}

/// (acceso?, secondi) del testo.
fn chiavi(st: &Stazione, text: &str, rng: &mut Rng) -> Vec<(bool, f64)> {
    let unit = 1.2 / st.wpm;
    let j = |x: f64, rng: &mut Rng| (x * (1.0 + st.jitter * rng.gauss())).max(0.4 * x);
    let mut seq = Vec::new();
    for c in text.bytes() {
        if c == b' ' {
            seq.push((false, j(4.0 * unit, rng)));
            continue;
        }
        let Some(p) = morse(c) else { continue };
        for (k, e) in p.bytes().enumerate() {
            if k > 0 {
                seq.push((false, j(unit, rng)));
            }
            seq.push((true, j(if e == b'-' { st.dash } else { 1.0 } * unit, rng)));
        }
        seq.push((false, j(3.0 * unit, rng)));
    }
    seq
}

fn durata(seq: &[(bool, f64)]) -> f64 {
    seq.iter().map(|s| s.1).sum()
}

/// Suona `seq` da `start_s` nella banda: fronti a coseno di 4 ms, QSB, deriva; la fase del tono segue la frequenza istantanea.
fn suona(band: &mut [f32], st: &Stazione, start_s: f64, seq: &[(bool, f64)]) {
    // potenza A²/2 contro il rumore in 2,5 kHz (sigma² · 2500 / 6000)
    let amp0 = SIGMA * (2.0 * 2_500.0 / (RATE / 2.0) * 10f64.powf(st.snr_db / 10.0)).sqrt();
    let ramp = 0.004 * RATE;
    let mut t_end = start_s * RATE;
    for &(on, secs) in seq {
        let start = t_end.round() as usize;
        t_end += secs * RATE;
        if !on {
            continue;
        }
        let len = (t_end.round() as usize).saturating_sub(start);
        for i in 0..len {
            let n = start + i;
            if n >= band.len() {
                return;
            }
            let t = n as f64 / RATE;
            let edge = i.min(len - 1 - i) as f64;
            let w = if edge < ramp { 0.5 * (1.0 - (PI * edge / ramp).cos()) } else { 1.0 };
            let qsb = 10f64.powf(-st.qsb_db / 20.0 * 0.5 * (1.0 - (2.0 * PI * t / st.qsb_s + st.qsb_ph).cos()));
            // fase: integrale della frequenza st.hz + drift·t
            let ph = 2.0 * PI * (st.hz * t + 0.5 * st.drift * t * t);
            band[n] += (amp0 * qsb * w * ph.sin()) as f32;
        }
    }
}

const PREFISSI: [&str; 24] = ["K", "W", "N", "DL", "G", "F", "EA", "OH", "JA", "VK", "SP", "OK", "HA", "YO", "LZ", "ON", "PA", "OE", "S5", "9A", "UA", "IK", "SM", "OZ"];

fn nominativo(rng: &mut Rng, usati: &mut HashSet<String>) -> String {
    loop {
        let p = PREFISSI[(rng.next_u64() % PREFISSI.len() as u64) as usize];
        let d = rng.next_u64() % 10;
        let n = 1 + (rng.next_u64() % 3) as usize;
        let s: String = (0..n).map(|_| char::from(b'A' + (rng.next_u64() % 26) as u8)).collect();
        let c = format!("{p}{d}{s}");
        if c.len() >= 4 && usati.insert(c.clone()) {
            return c;
        }
    }
}

fn tra(rng: &mut Rng, a: f64, b: f64) -> f64 {
    a + (b - a) * rng.uniform()
}

fn stazione(rng: &mut Rng, call: String, hz: f64, wpm: f64) -> Stazione {
    let jitter = if rng.uniform() < 0.25 { tra(rng, 0.06, 0.12) } else { tra(rng, 0.0, 0.04) };
    Stazione {
        call,
        hz,
        wpm,
        snr_db: tra(rng, -12.0, 20.0),
        jitter,
        dash: if jitter > 0.05 { tra(rng, 3.0, 3.6) } else { 3.0 },
        qsb_db: if rng.uniform() < 0.5 { tra(rng, 3.0, 15.0) } else { 0.0 },
        qsb_s: tra(rng, 5.0, 20.0),
        qsb_ph: tra(rng, 0.0, 2.0 * PI),
        drift: tra(rng, -0.05, 0.05),
    }
}

/// Chi chiama CQ in una banda: la stazione, i momenti di fine dei suoi CQ e la fine dell'ultima trasmissione.
struct Runner {
    st: Stazione,
    cq_end: Vec<f64>,
    last_s: f64,
}

/// Una banda: l'audio, chi chiama CQ e tutti i nominativi presenti.
fn banda(seed: u64, n_runner: usize, dur_s: f64) -> (Vec<f32>, Vec<Runner>, HashSet<String>) {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut band = vec![0.0f32; (dur_s * RATE) as usize];
    common::aggiungi_rumore(&mut band, SIGMA, &mut rng);
    let mut usati = HashSet::new();
    // frequenze: a gruppi, con le vicine a 30-80 Hz
    let mut freqs = Vec::new();
    let mut f = tra(&mut rng, 350.0, 450.0);
    while freqs.len() < n_runner {
        freqs.push(f);
        f += if rng.uniform() < 0.5 { tra(&mut rng, 30.0, 80.0) } else { tra(&mut rng, 120.0, 220.0) };
    }
    let mut runners = Vec::new();
    let mut tutti = HashSet::new();
    for (i, &hz) in freqs.iter().enumerate() {
        // un terzo veloci (33-40), gli altri fra 15 e 32
        let wpm = if i % 3 == 0 { tra(&mut rng, 33.0, 40.0) } else { tra(&mut rng, 15.0, 32.0) };
        let call = nominativo(&mut rng, &mut usati);
        tutti.insert(call.clone());
        let st = stazione(&mut rng, call.clone(), hz, wpm);
        let mut t = tra(&mut rng, 0.5, 30.0);
        let fine = tra(&mut rng, 0.6 * dur_s, dur_s);
        let mut cq_end = Vec::new();
        let mut nr = 1 + (rng.next_u64() % 300) as u32;
        let mut last_s = t;
        while t < fine {
            let cq = if rng.uniform() < 0.7 { format!("CQ TEST {call} {call}") } else { format!("CQ {call} {call} TEST") };
            let seq = chiavi(&st, &cq, &mut rng);
            suona(&mut band, &st, t, &seq);
            t += durata(&seq);
            cq_end.push(t);
            last_s = t;
            t += tra(&mut rng, 0.5, 1.5);
            if rng.uniform() < 0.65 {
                // chi risponde: sulla frequenza del runner o quasi, con la sua velocita' e il suo S/N
                let cc = nominativo(&mut rng, &mut usati);
                tutti.insert(cc.clone());
                let cw = tra(&mut rng, 18.0, 38.0);
                let off = tra(&mut rng, -30.0, 30.0);
                let ca = stazione(&mut rng, cc.clone(), hz + off, cw);
                let txt = if rng.uniform() < 0.3 { format!("{cc} {cc}") } else { cc.clone() };
                let s = chiavi(&ca, &txt, &mut rng);
                suona(&mut band, &ca, t, &s);
                t += durata(&s) + tra(&mut rng, 0.3, 0.8);
                let s = chiavi(&st, &format!("{cc} 5NN {nr:03}"), &mut rng);
                suona(&mut band, &st, t, &s);
                t += durata(&s) + tra(&mut rng, 0.3, 0.8);
                nr += 1;
                let txt = if rng.uniform() < 0.5 { format!("TU 5NN {:03}", 1 + rng.next_u64() % 999) } else { format!("R 5NN {}", 1 + rng.next_u64() % 999) };
                let s = chiavi(&ca, &txt, &mut rng);
                suona(&mut band, &ca, t, &s);
                t += durata(&s) + tra(&mut rng, 0.3, 0.8);
                let s = chiavi(&st, &format!("TU {call} TEST"), &mut rng);
                suona(&mut band, &st, t, &s);
                t += durata(&s);
                last_s = t;
                t += tra(&mut rng, 0.5, 1.5);
            } else {
                t += tra(&mut rng, 1.0, 3.0);
            }
        }
        runners.push(Runner { st, cq_end, last_s: last_s.min(dur_s) });
    }
    (band, runners, tutti)
}

struct Snap {
    t: f64,
    /// nominativo -> frequenza: CQ verificati, non vecchi e non spariti (come `radarStations` di `digiBand.ts`).
    radar: HashMap<String, f32>,
    /// Tutti i nominativi verificati, con qualunque ruolo.
    verified: HashSet<String>,
    /// Frequenze dei canali aperti e il loro testo (per `CW_DEBUG`).
    lanes: Vec<(f32, String)>,
}

fn snapshot(d: &dyn BandDecoder, t: f64) -> Snap {
    let mut radar = HashMap::new();
    let mut verified = HashSet::new();
    let chans = d.channels();
    let lanes = chans.iter().map(|c| (c.freq_hz, c.text.clone())).collect();
    for ch in &chans {
        for c in &ch.calls {
            let latest = ch
                .calls
                .iter()
                .filter(|o| o.role != Role::Other && o.verified() && (c.role != Role::Cq || (o.role == Role::Cq && (o.cq_count >= 2 || !c.verified()))))
                .map(|o| o.last_s)
                .fold(f64::MIN, f64::max);
            let replaced = c.role != Role::Other && latest > c.last_s;
            let behind = latest - c.last_s;
            let silent = ch.now_s - c.last_s;
            let stale_after = if c.role == Role::Cq && c.verified() { CALL_STALE_CQ_S } else { CALL_STALE_S };
            let stale = silent > stale_after || (replaced && behind > SUPERSEDED_S);
            let gone = silent > CALL_GONE_S || (replaced && behind > SUPERSEDED_GONE_S);
            if c.verified() {
                verified.insert(c.call.clone());
                if c.role == Role::Cq && !stale && !gone {
                    radar.insert(c.call.clone(), ch.freq_hz);
                }
            }
        }
    }
    Snap { t, radar, verified, lanes }
}

/// I conti di una o piu' bande.
#[derive(Default)]
struct Conti {
    runner: usize,
    trovati: usize,
    pres_sum: f64,
    ritardi: Vec<f64>,
    drops: usize,
    falsi_cq: usize,
    falsi_verif: usize,
    /// Per fascia di velocita' (<25, 25-33, >=33): (runner, trovati, somma presenze).
    fasce: [(usize, usize, f64); 3],
}

fn fascia(wpm: f64) -> usize {
    if wpm < 25.0 {
        0
    } else if wpm < 33.0 {
        1
    } else {
        2
    }
}

fn misura_banda(seed: u64, n_runner: usize, dur_s: f64, conti: &mut Conti) {
    let (audio, runners, tutti) = banda(seed, n_runner, dur_s);
    let mut dec = CwBandDecoder::new(150.0, 4_000.0, 40);
    let mut snaps = Vec::new();
    let (mut t, mut next) = (0.0, SNAP_S);
    let started = std::time::Instant::now();
    let mut max_lanes = 0;
    for block in audio.chunks(1_200) {
        dec.process(block);
        t += block.len() as f64 / RATE;
        max_lanes = max_lanes.max(dec.channel_count());
        if t >= next {
            snaps.push(snapshot(&dec, t));
            next += SNAP_S;
        }
    }
    eprintln!("banda {seed}: {:.0} s di calcolo per {dur_s:.0} s d'audio, canali al massimo {max_lanes}", started.elapsed().as_secs_f64());
    println!("{:<9} {:>6} {:>4} {:>5} {:>4} {:>6} {:>6} {:>5} {:>6}", "call", "Hz", "WPM", "S/N", "QSB", "ritardo", "%pres", "drop", "%lane");
    let runner_calls: HashSet<&str> = runners.iter().map(|r| r.st.call.as_str()).collect();
    for r in &runners {
        let Some(&first) = r.cq_end.first() else { continue };
        let call = r.st.call.as_str();
        let active = |t: f64| t >= first && t <= r.last_s + 30.0;
        let first_band = snaps.iter().find(|s| s.t >= first - 2.0 && s.radar.contains_key(call)).map(|s| s.t);
        let (mut n, mut pres, mut drops, mut was, mut lane) = (0, 0, 0, false, 0);
        let mut testo = String::new();
        for s in snaps.iter().filter(|s| active(s.t)) {
            n += 1;
            if let Some(l) = s.lanes.iter().filter(|l| (f64::from(l.0) - r.st.hz).abs() < 15.0).min_by(|a, b| (f64::from(a.0) - r.st.hz).abs().total_cmp(&(f64::from(b.0) - r.st.hz).abs())) {
                lane += 1;
                testo.clone_from(&l.1);
            }
            let p = s.radar.contains_key(call);
            if p {
                pres += 1;
            } else if was {
                drops += 1;
            }
            was = p;
        }
        let pct = if n > 0 { 100.0 * f64::from(pres) / f64::from(n) } else { 0.0 };
        conti.runner += 1;
        conti.pres_sum += pct;
        conti.drops += drops;
        let fa = &mut conti.fasce[fascia(r.st.wpm)];
        fa.0 += 1;
        fa.2 += pct;
        if let Some(fb) = first_band {
            conti.trovati += 1;
            fa.1 += 1;
            conti.ritardi.push((fb - first).max(0.0));
        }
        println!(
            "{:<9} {:>6.0} {:>4.0} {:>5.1} {:>4.0} {:>6} {:>5.0}% {:>5} {:>5.0}%",
            call,
            r.st.hz,
            r.st.wpm,
            r.st.snr_db,
            r.st.qsb_db,
            first_band.map(|fb| format!("{:.0} s", (fb - first).max(0.0))).unwrap_or_else(|| "-".into()),
            pct,
            drops,
            if n > 0 { 100.0 * f64::from(lane) / f64::from(n) } else { 0.0 }
        );
        if std::env::var_os("CW_DEBUG").is_some() && first_band.is_none() {
            let tail: String = testo.chars().rev().take(120).collect::<Vec<char>>().into_iter().rev().collect();
            println!("      {tail}");
        }
    }
    let in_radar: HashSet<&String> = snaps.iter().flat_map(|s| s.radar.keys()).collect();
    let falsi: Vec<&&String> = in_radar.iter().filter(|c| !runner_calls.contains(c.as_str())).collect();
    let all_verif: HashSet<&String> = snaps.iter().flat_map(|s| s.verified.iter()).collect();
    let falsi_v: Vec<&&String> = all_verif.iter().filter(|c| !tutti.contains(c.as_str())).collect();
    println!("falsi nell'elenco come CQ: {falsi:?} · verificati inesistenti: {falsi_v:?}");
    conti.falsi_cq += falsi.len();
    conti.falsi_verif += falsi_v.len();
    if std::env::var_os("CW_DEBUG").is_some() {
        for ch in dec.channels() {
            let v: Vec<String> = ch.calls.iter().map(|c| format!("{}:{:?}{}", c.call, c.role, if c.verified() { "v" } else { "" })).collect();
            let tail: String = ch.text.chars().rev().take(90).collect::<Vec<char>>().into_iter().rev().collect();
            println!("   {:7.1} Hz {:4.0} WPM {:5.1} dB {v:?}
      {tail}", ch.freq_hz, ch.wpm, ch.snr_db);
        }
    }
}

#[test]
#[ignore]
fn banda_cw_sintetica() {
    let semi: u64 = std::env::var("CW_BANCO_SEMI").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    let dur: f64 = std::env::var("CW_BANCO_DURATA").ok().and_then(|s| s.parse().ok()).unwrap_or(240.0);
    let n_runner: usize = std::env::var("CW_BANCO_RUNNER").ok().and_then(|s| s.parse().ok()).unwrap_or(16);
    let mut conti = Conti::default();
    let primo: u64 = std::env::var("CW_BANCO_SEME").ok().and_then(|s| s.parse().ok()).unwrap_or(1_000);
    for s in 0..semi {
        misura_banda(primo + s, n_runner, dur, &mut conti);
    }
    conti.ritardi.sort_by(f64::total_cmp);
    let mediana = conti.ritardi.get(conti.ritardi.len() / 2).map(|m| format!("{m:.0} s")).unwrap_or_else(|| "-".into());
    let fasce: Vec<String> = ["<25", "25-33", ">=33"]
        .iter()
        .zip(&conti.fasce)
        .map(|(nome, (n, tr, p))| format!("{nome} WPM {tr}/{n} {:.0}%", if *n > 0 { p / *n as f64 } else { 0.0 }))
        .collect();
    println!("fasce di velocita' (trovati, presenza): {}", fasce.join(" | "));
    println!(
        "RIEPILOGO cw sintetico: trovati {}/{} | presenza {:.0}% | ritardo mediano {mediana} | interruzioni {} | falsi CQ {} | verificati inesistenti {}",
        conti.trovati,
        conti.runner,
        conti.pres_sum / conti.runner.max(1) as f64,
        conti.drops,
        conti.falsi_cq,
        conti.falsi_verif
    );
}
