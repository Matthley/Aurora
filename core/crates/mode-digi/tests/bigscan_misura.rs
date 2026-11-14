//! Banco di prova di «tutta la banda» («BIGScan»): quanto bene il decodificatore di tutta la banda trova chi chiama CQ, rispetto a un
//! ricevitore a canale singolo per ogni segnale, su una registrazione vera.
//! - `AURORA_BIGSCAN_WAV`: il file WAV (mono, PCM 16 bit, qualunque frequenza); senza, la prova si salta;
//! - `AURORA_BIGSCAN_MODE`: `rtty` (di base, 170 Hz, 45,45 baud) o `cw`;
//! - `AURORA_BIGSCAN_SNAP_S`: ogni quanti secondi d'audio si fotografa l'elenco (di base 5; 1 si avvicina al rinfresco vero
//!   dell'interfaccia e toglie al ritardo la mezza griglia delle fotografie).
//!
//! L'audio passa come nel servizio (ricampionatore di `mode-sstv` a 12 kHz, blocchi di 100 ms); ogni 5 s d'audio si fotografa l'elenco
//! che mostrerebbe l'interfaccia (nominativi verificati dopo CQ, non vecchi e non spariti, come `radarStations` di `digiBand.ts`).
//! Stampa la tabella dei chiamanti e una riga di riepilogo da confrontare fra le versioni; la tabella va anche in
//! `<wav>.bigscan-<modo>.txt` accanto al file.
//!
//! `CARGO_BUILD_JOBS=2 nice cargo test --release -j 2 -p mode-digi --test bigscan_misura -- --ignored --nocapture --test-threads=1`

use mode_digi::cw::CwDemodulator;
use mode_digi::rtty::{RttyDemodulator, RttyParams};
use mode_digi::skim::calls::Role;
use mode_digi::skim::rtty::RttyBandDecoder;
use mode_digi::skim::{BandDecoder, CwBandDecoder};
use mode_sstv::resample::Resampler;
use rustfft::{FftPlanner, num_complex::Complex};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

const RATE: f64 = 12_000.0;
const SHIFT: f32 = 170.0;
const BAUD: f32 = 45.45;
const SNAP_S: f64 = 5.0;
fn snap_s() -> f64 {
    std::env::var("AURORA_BIGSCAN_SNAP_S").ok().and_then(|v| v.parse::<f64>().ok()).filter(|v| *v > 0.0).unwrap_or(SNAP_S)
}
/// Come `band.rs`.
const CALL_STALE_S: f64 = 75.0;
const CALL_STALE_CQ_S: f64 = 180.0;
const SUPERSEDED_S: f64 = 10.0;
const SUPERSEDED_GONE_S: f64 = 60.0;
const CALL_GONE_S: f64 = 180.0;

fn leggi_wav_mono16(path: &Path) -> (u32, Vec<f32>) {
    let b = std::fs::read(path).unwrap();
    assert!(&b[0..4] == b"RIFF" && &b[8..12] == b"WAVE");
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
            assert_eq!(bits, 16, "serve PCM a 16 bit");
            let end = (body + len).min(b.len());
            let x = b[body..end].chunks_exact(2 * canali).map(|s| f32::from(i16::from_le_bytes([s[0], s[1]])) / 32768.0).collect();
            return (rate, x);
        }
        i = body + len + (len & 1);
    }
    panic!("manca il blocco data");
}

/// Forma di nominativo come `CALL_SHAPE` di `digiBand.ts`.
fn is_call(w: &str) -> bool {
    let b = w.as_bytes();
    if b.len() < 4 || b.len() > 9 || !b.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        return false;
    }
    if !b[b.len() - 1].is_ascii_uppercase() {
        return false;
    }
    // [A-Z0-9]{1,3} \d [A-Z0-9]{0,4} [A-Z]
    (1..=3).any(|p| p < b.len() - 1 && b[p].is_ascii_digit() && b.len() - 1 - (p + 1) <= 4)
}

#[derive(Default, Clone)]
struct BandCall {
    lane_hz: f32,
    role: &'static str,
    verified: bool,
    stale: bool,
    gone: bool,
}

struct Snap {
    t: f64,
    lanes: Vec<f32>,
    /// Tutti i nominativi dei canali, con la stessa logica di `band.rs::signal_of`.
    calls: Vec<(String, BandCall)>,
}

impl Snap {
    fn radar(&self) -> HashMap<String, f32> {
        let mut m = HashMap::new();
        for (c, b) in &self.calls {
            if b.verified && !b.stale && !b.gone && b.role == "cq" {
                m.insert(c.clone(), b.lane_hz);
            }
        }
        m
    }
}

fn snapshot(d: &dyn BandDecoder, t: f64) -> Snap {
    let chans = d.channels();
    let mut calls = Vec::new();
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
            let role = match c.role {
                Role::Cq => "cq",
                Role::De => "de",
                Role::Other => "other",
            };
            calls.push((c.call.clone(), BandCall { lane_hz: ch.freq_hz, role, verified: c.verified(), stale, gone }));
        }
    }
    Snap { t, lanes: chans.iter().map(|c| c.freq_hz).collect(), calls }
}

/// Centri dei segnali: per segmenti di 10 s, spettro medio (FFT 4096); RTTY: coppie di picchi a 170 Hz sopra il fondo; CW: picchi.
fn trova_segnali(x: &[f32], cw: bool) -> Vec<(f32, usize)> {
    const N: usize = 4096;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let hz = RATE as f32 / N as f32;
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
    let seg = (10.0 * RATE) as usize;
    let mut votes: BTreeMap<i32, usize> = BTreeMap::new();
    let lo = (200.0 / hz) as usize;
    let hi = (4000.0 / hz) as usize;
    let step = (SHIFT / hz).round() as usize;
    for s in x.chunks(seg) {
        let mut acc = vec![0.0f32; N / 2];
        let mut n = 0;
        for f in s.chunks_exact(N / 2).collect::<Vec<_>>().windows(2) {
            let mut buf: Vec<Complex<f32>> = f[0].iter().chain(f[1].iter()).zip(&win).map(|(a, w)| Complex::new(a * w, 0.0)).collect();
            fft.process(&mut buf);
            for (a, c) in acc.iter_mut().zip(&buf) {
                *a += c.norm_sqr();
            }
            n += 1;
        }
        if n == 0 {
            continue;
        }
        let mut sorted = acc[lo..hi].to_vec();
        sorted.sort_by(f32::total_cmp);
        let floor = sorted[sorted.len() / 2];
        let peak = |k: usize| (k - 3..=k + 3).all(|q| acc[q] <= acc[k]);
        if cw {
            for i in lo..hi {
                if acc[i] > 10.0 * floor && peak(i) {
                    *votes.entry((i as f32 * hz / 10.0).round() as i32).or_default() += 1;
                }
            }
            continue;
        }
        for i in lo..hi - step {
            for j in [i + step - 1, i + step, i + step + 1] {
                if acc[i] > 6.0 * floor && acc[j] > 6.0 * floor && peak(i) && peak(j) {
                    let c = (i + j) as f32 / 2.0 * hz;
                    *votes.entry((c / 10.0).round() as i32).or_default() += 1;
                }
            }
        }
    }
    // gruppi di caselle da 10 Hz vicine (entro 30 Hz)
    let mut out: Vec<(f32, usize)> = Vec::new();
    let mut cur: Vec<(i32, usize)> = Vec::new();
    let flush = |cur: &mut Vec<(i32, usize)>, out: &mut Vec<(f32, usize)>| {
        if cur.is_empty() {
            return;
        }
        let tot: usize = cur.iter().map(|c| c.1).sum();
        let best = cur.iter().max_by_key(|c| c.1).map(|c| c.0).unwrap_or(0);
        out.push((best as f32 * 10.0, tot));
        cur.clear();
    };
    for (&k, &v) in &votes {
        if cur.last().is_some_and(|l| k - l.0 > 3) {
            flush(&mut cur, &mut out);
        }
        cur.push((k, v));
    }
    flush(&mut cur, &mut out);
    out.retain(|o| o.1 >= 3);
    out
}

/// CQ dal testo di un canale singolo: (momento, nominativo) per ogni riga con CQ/TEST/QRZ e un nominativo ripetuto almeno due volte.
fn cq_del_testo(chunks: &[(f64, Vec<u8>)]) -> Vec<(f64, String)> {
    let mut lines: Vec<(f64, String)> = Vec::new();
    let mut cur = String::new();
    let mut t0 = 0.0;
    let mut last = 0.0;
    for (t, bytes) in chunks {
        if !cur.is_empty() && t - last > 3.0 {
            lines.push((t0, std::mem::take(&mut cur)));
        }
        for &b in bytes {
            if b == b'\r' || b == b'\n' || cur.len() > 80 {
                if !cur.is_empty() {
                    lines.push((t0, std::mem::take(&mut cur)));
                }
                if b == b'\r' || b == b'\n' {
                    continue;
                }
            }
            if cur.is_empty() {
                t0 = *t;
            }
            cur.push(char::from(b).to_ascii_uppercase());
        }
        if !bytes.is_empty() {
            last = *t;
        }
    }
    if !cur.is_empty() {
        lines.push((t0, cur));
    }
    let mut out = Vec::new();
    for (t, l) in lines {
        let words: Vec<&str> = l.split(|c: char| !c.is_ascii_alphanumeric() && c != '/').filter(|w| !w.is_empty()).collect();
        if !words.iter().any(|w| matches!(*w, "CQ" | "TEST" | "QRZ")) {
            continue;
        }
        let mut n: HashMap<&str, usize> = HashMap::new();
        for w in &words {
            if is_call(w) {
                *n.entry(w).or_default() += 1;
            }
        }
        for (c, k) in n {
            if k >= 2 {
                out.push((t, c.to_string()));
            }
        }
    }
    out
}

/// Coppie RTTY presenti nel file: per blocchi di 30 s, spettro medio (FFT 4096) e fondo locale (30° percentile su 400 Hz attorno
/// a ogni casella); una coppia sono due picchi a 170 Hz entrambi sopra il fondo locale, di livello simile, con il centro fra i due
/// basso. Torna (centro, blocchi da 30 s in cui la coppia c'e').
const BLOCK_S: f64 = 30.0;
const SUB_FRAMES: usize = 12;
const PAIR_RATIO: f32 = 3.0;
fn trova_coppie(x: &[f32]) -> Vec<(f32, Vec<usize>)> {
    const N: usize = 4096;
    let fft = FftPlanner::<f32>::new().plan_fft_forward(N);
    let hz = RATE as f32 / N as f32;
    let win: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / N as f32).cos()).collect();
    let seg = (BLOCK_S * RATE) as usize;
    let lo = (250.0 / hz) as usize;
    let hi = (3900.0 / hz) as usize;
    let step = (SHIFT / hz).round() as usize;
    let half = (200.0 / hz) as usize;
    // (centro in caselle da 5 Hz, blocco)
    let mut hits: Vec<(i32, usize)> = Vec::new();
    for (b, s) in x.chunks(seg).enumerate() {
        // massimo per casella delle medie su finestre di ~2 s (un CQ breve conta quanto un segnale continuo)
        let mut acc = vec![0.0f32; N / 2];
        let mut sub = vec![0.0f32; N / 2];
        let mut n = 0;
        let mut subs = 0;
        for f in s.chunks_exact(N / 2).collect::<Vec<_>>().windows(2) {
            let mut buf: Vec<Complex<f32>> = f[0].iter().chain(f[1].iter()).zip(&win).map(|(a, w)| Complex::new(a * w, 0.0)).collect();
            fft.process(&mut buf);
            for (a, c) in sub.iter_mut().zip(&buf) {
                *a += c.norm_sqr();
            }
            n += 1;
            if n % SUB_FRAMES == 0 {
                for (a, v) in acc.iter_mut().zip(sub.iter_mut()) {
                    *a = a.max(*v);
                    *v = 0.0;
                }
                subs += 1;
            }
        }
        if subs < 2 {
            continue;
        }
        // fondo locale a passi di 8 caselle
        let mut floor = vec![0.0f32; N / 2];
        let mut k = lo.saturating_sub(half);
        while k < (hi + half).min(N / 2) {
            let a = k.saturating_sub(half).max(1);
            let z = (k + half).min(N / 2);
            let mut v = acc[a..z].to_vec();
            v.sort_by(f32::total_cmp);
            let f = v[v.len() * 3 / 10];
            for q in k..(k + 8).min(N / 2) {
                floor[q] = f;
            }
            k += 8;
        }
        let peak = |k: usize| (k - 3..=k + 3).all(|q| acc[q] <= acc[k]);
        for i in lo..hi - step - 1 {
            if !(acc[i] > PAIR_RATIO * floor[i] && peak(i)) {
                continue;
            }
            for j in [i + step - 1, i + step, i + step + 1] {
                if !(acc[j] > PAIR_RATIO * floor[j] && peak(j)) {
                    continue;
                }
                let (a, c) = (acc[i].min(acc[j]), acc[i].max(acc[j]));
                let mid = acc[(i + j) / 2];
                if c > 12.0 * a || mid > 0.7 * a {
                    continue;
                }
                let centre = (i + j) as f32 / 2.0 * hz;
                hits.push(((centre / 5.0).round() as i32, b));
            }
        }
    }
    // gruppi di centri vicini (entro 20 Hz fra caselle consecutive)
    hits.sort();
    let mut out: Vec<(f32, Vec<usize>)> = Vec::new();
    let mut i = 0;
    while i < hits.len() {
        let mut j = i + 1;
        while j < hits.len() && hits[j].0 - hits[j - 1].0 <= 4 {
            j += 1;
        }
        let g = &hits[i..j];
        let mut cs: Vec<i32> = g.iter().map(|h| h.0).collect();
        cs.sort();
        let centre = cs[cs.len() / 2] as f32 * 5.0;
        let mut blocks: Vec<usize> = g.iter().map(|h| h.1).collect();
        blocks.sort();
        blocks.dedup();
        if !blocks.is_empty() {
            out.push((centre, blocks));
        }
        i = j;
    }
    out
}

/// Righe di testo di un canale singolo: (momento d'inizio, testo maiuscolo).
fn righe(chunks: &[(f64, Vec<u8>)]) -> Vec<(f64, String)> {
    let mut lines: Vec<(f64, String)> = Vec::new();
    let mut cur = String::new();
    let mut t0 = 0.0;
    let mut last = 0.0;
    for (t, bytes) in chunks {
        if !cur.is_empty() && t - last > 3.0 {
            lines.push((t0, std::mem::take(&mut cur)));
        }
        for &b in bytes {
            if b == b'\r' || b == b'\n' || cur.len() > 80 {
                if !cur.is_empty() {
                    lines.push((t0, std::mem::take(&mut cur)));
                }
                if b == b'\r' || b == b'\n' {
                    continue;
                }
            }
            if cur.is_empty() {
                t0 = *t;
            }
            cur.push(char::from(b).to_ascii_uppercase());
        }
        if !bytes.is_empty() {
            last = *t;
        }
    }
    if !cur.is_empty() {
        lines.push((t0, cur));
    }
    lines
}

fn parole(l: &str) -> Vec<&str> {
    l.split(|c: char| !c.is_ascii_alphanumeric() && c != '/').filter(|w| !w.is_empty()).collect()
}

/// CQ dal testo (regola morbida): una riga con CQ o TEST e un nominativo che compare due volte nella riga stessa o anche in una
/// delle due righe vicine (prima o dopo, entro 15 s).
fn cq_morbidi(lines: &[(f64, String)]) -> Vec<(f64, String)> {
    let ws: Vec<Vec<&str>> = lines.iter().map(|l| parole(&l.1)).collect();
    let mut out = Vec::new();
    for (k, w) in ws.iter().enumerate() {
        if !w.iter().any(|w| matches!(*w, "CQ" | "TEST")) {
            continue;
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for c in w.iter().copied().filter(|w| is_call(w)) {
            if !seen.insert(c) {
                continue;
            }
            let here = w.iter().filter(|x| **x == c).count();
            let near = [k.wrapping_sub(1), k + 1]
                .iter()
                .filter(|&&q| q < ws.len() && (lines[q].0 - lines[k].0).abs() < 15.0)
                .any(|&q| ws[q].contains(&c));
            if here >= 2 || near {
                out.push((lines[k].0, c.to_string()));
            }
        }
    }
    out
}

/// Il riferimento RTTY: un ricevitore a canale singolo per ogni coppia trovata, acceso solo nei blocchi in cui la coppia c'e' (piu'
/// uno prima e uno dopo); un nominativo va alla sola coppia che lo nomina di piu'. Torna nominativo -> (frequenza, momenti dei CQ).
fn riferimento_rtty(audio: &[f32]) -> BTreeMap<String, (f32, Vec<f64>)> {
    let pairs = trova_coppie(audio);
    eprintln!("coppie RTTY trovate: {}", pairs.len());
    let seg = (BLOCK_S * RATE) as usize;
    let nblocks = audio.len().div_ceil(seg);
    // nominativo -> coppia -> (righe in cui compare, momenti dei CQ)
    let mut per_call: BTreeMap<String, BTreeMap<usize, (usize, Vec<f64>)>> = BTreeMap::new();
    for (p, (centre, blocks)) in pairs.iter().enumerate() {
        let mut on = vec![false; nblocks];
        for &b in blocks {
            for q in b.saturating_sub(1)..=(b + 1).min(nblocks - 1) {
                on[q] = true;
            }
        }
        let mut chunks = Vec::new();
        let mut b = 0;
        while b < nblocks {
            if !on[b] {
                b += 1;
                continue;
            }
            let start = b;
            while b < nblocks && on[b] {
                b += 1;
            }
            let mut d = RttyDemodulator::new(RttyParams { centre_hz: *centre, shift_hz: SHIFT, baud: BAUD, reverse: false });
            let a = start * seg;
            let z = (b * seg).min(audio.len());
            let mut t = a as f64 / RATE;
            for block in audio[a..z].chunks(1200) {
                let out = d.process(block);
                t += block.len() as f64 / RATE;
                chunks.push((t, out));
            }
            chunks.push((t + 10.0, Vec::new()));
        }
        let lines = righe(&chunks);
        let mut mentions: HashMap<&str, usize> = HashMap::new();
        for (_, l) in &lines {
            let mut seen = HashSet::new();
            for w in parole(l) {
                if is_call(w) && seen.insert(w) {
                    *mentions.entry(w).or_default() += 1;
                }
            }
        }
        let cqs = cq_morbidi(&lines);
        let names: HashSet<&str> = cqs.iter().map(|c| c.1.as_str()).collect();
        eprintln!("  {centre:6.0} Hz ({:3} blocchi): {} CQ, {:?}", blocks.len(), cqs.len(), names);
        for (t, c) in &cqs {
            let m = mentions.get(c.as_str()).copied().unwrap_or(0);
            let e = per_call.entry(c.clone()).or_default().entry(p).or_insert((m, Vec::new()));
            e.1.push(*t);
        }
    }
    let mut single = BTreeMap::new();
    for (call, by_pair) in per_call {
        // la coppia con piu' righe che lo nominano (a pari merito quella con piu' CQ) da' la frequenza; le altre contano solo se lo
        // nominano almeno un terzo delle volte (la radio risintonizzata sposta la stazione), non se lo leggono di sbieco
        let Some((&p, &(best, _))) = by_pair.iter().max_by_key(|(_, (m, t))| (*m, t.len())) else { continue };
        let mut times: Vec<f64> = by_pair.values().filter(|(m, _)| 3 * m >= best).flat_map(|(_, t)| t.iter().copied()).collect();
        times.sort_by(f64::total_cmp);
        if !times.is_empty() {
            single.insert(call, (pairs[p].0, times));
        }
    }
    single
}

/// Il riferimento letto dalla copia accanto al WAV, se c'e' ed e' della stessa versione.
const RIF_VER: &str = "rif-v5";
fn leggi_rif(path: &str) -> Option<BTreeMap<String, (f32, Vec<f64>)>> {
    let s = std::fs::read_to_string(path).ok()?;
    let mut it = s.lines();
    if it.next()? != RIF_VER {
        return None;
    }
    let mut m = BTreeMap::new();
    for l in it {
        let mut f = l.split(' ');
        let call = f.next()?.to_string();
        let hz: f32 = f.next()?.parse().ok()?;
        let times: Vec<f64> = f.filter_map(|t| t.parse().ok()).collect();
        m.insert(call, (hz, times));
    }
    Some(m)
}

fn scrivi_rif(path: &str, m: &BTreeMap<String, (f32, Vec<f64>)>) {
    let mut s = format!("{RIF_VER}\n");
    for (c, (hz, ts)) in m {
        let _ = write!(s, "{c} {hz}");
        for t in ts {
            let _ = write!(s, " {t:.1}");
        }
        s.push('\n');
    }
    let _ = std::fs::write(path, s);
}

/// Il riferimento CW: un ricevitore per ogni picco trovato.
fn riferimento_cw(audio: &[f32]) -> BTreeMap<String, (f32, Vec<f64>)> {
    let cw = true;
    let sig = trova_segnali(audio, cw);
    eprintln!("segnali trovati: {}", sig.len());
    // nominativo -> (frequenza, momenti in cui chiama CQ)
    let mut single: BTreeMap<String, (f32, Vec<f64>)> = BTreeMap::new();
    for &(centre, votes) in &sig {
        let mut rtty = (!cw).then(|| RttyDemodulator::new(RttyParams { centre_hz: centre, shift_hz: SHIFT, baud: BAUD, reverse: false }));
        let mut morse = cw.then(|| CwDemodulator::new(centre, 25.0));
        let mut chunks = Vec::new();
        let mut t = 0.0;
        for block in audio.chunks(1200) {
            let out = match (&mut rtty, &mut morse) {
                (Some(d), _) => d.process(block),
                (_, Some(d)) => d.process(block),
                _ => Vec::new(),
            };
            t += block.len() as f64 / RATE;
            chunks.push((t, out));
        }
        let cqs = cq_del_testo(&chunks);
        let calls: HashSet<&str> = cqs.iter().map(|c| c.1.as_str()).collect();
        eprintln!("  {centre:6.0} Hz ({votes:3} segm.): {} CQ, {:?}", cqs.len(), calls);
        for (t, c) in cqs {
            let e = single.entry(c).or_insert((centre, Vec::new()));
            e.1.push(t);
        }
    }
    // una sola ricorrenza puo' essere un errore: si tengono quelli sentiti almeno 2 volte
    single.retain(|_, v| v.1.len() >= 2);

    single
}

fn mmss(t: f64) -> String {
    format!("{:02}:{:02}", (t / 60.0) as u32, (t % 60.0) as u32)
}

#[test]
#[ignore]
fn bigscan_contro_canale_singolo() {
    let Ok(path) = std::env::var("AURORA_BIGSCAN_WAV") else {
        eprintln!("AURORA_BIGSCAN_WAV non impostata: prova saltata");
        return;
    };
    let cw = std::env::var("AURORA_BIGSCAN_MODE").is_ok_and(|m| m.eq_ignore_ascii_case("cw"));
    let mode = if cw { "cw" } else { "rtty" };
    let (rate, raw) = leggi_wav_mono16(Path::new(&path));
    // come il servizio: Resampler di mode-sstv a 12 kHz, blocchi da ~100 ms
    let mut rs = Resampler::new(rate);
    let mut audio = Vec::new();
    let mut tmp = Vec::new();
    for c in raw.chunks((rate / 10).max(1) as usize) {
        tmp.clear();
        rs.process(c, &mut tmp);
        audio.extend_from_slice(&tmp);
    }
    drop(raw);
    let dur = audio.len() as f64 / RATE;
    eprintln!("{mode}: {:.1} min a 12 kHz", dur / 60.0);

    // 1. tutta la banda, come il thread di band.rs (senza limite di CPU e senza cty.dat)
    let started = std::time::Instant::now();
    let mut band: Box<dyn BandDecoder> =
        if cw { Box::new(CwBandDecoder::new(150.0, 4_000.0, 40)) } else { Box::new(RttyBandDecoder::new(SHIFT, BAUD, 40)) };
    let mut snaps: Vec<Snap> = Vec::new();
    let mut t = 0.0;
    let snap = snap_s();
    let mut next_snap = snap;
    let mut max_lanes = 0;
    for block in audio.chunks(1200) {
        band.process(block);
        t += block.len() as f64 / RATE;
        max_lanes = max_lanes.max(band.channel_count());
        if t >= next_snap {
            snaps.push(snapshot(band.as_ref(), t));
            next_snap += snap;
        }
    }
    eprintln!("tutta la banda: {:.0} s di calcolo, canali al massimo {max_lanes}", started.elapsed().as_secs_f64());

    // 2. il riferimento: RTTY una coppia alla volta (copia in `<wav>.rif-rtty.txt`, si rifa' se cambia RIF_VER); CW un segnale alla volta
    let started = std::time::Instant::now();
    let rif_path = format!("{path}.rif-{mode}.txt");
    let single = if cw {
        riferimento_cw(&audio)
    } else if let Some(m) = leggi_rif(&rif_path) {
        eprintln!("riferimento letto da {rif_path}");
        m
    } else {
        let m = riferimento_rtty(&audio);
        scrivi_rif(&rif_path, &m);
        m
    };
    eprintln!("canali singoli: {:.0} s di calcolo", started.elapsed().as_secs_f64());

    // 3. tabella
    let radar: Vec<HashMap<String, f32>> = snaps.iter().map(Snap::radar).collect();
    let mut rep = String::new();
    writeln!(rep, "BIGScan {mode}: {path}").unwrap();
    writeln!(
        rep,
        "{:<10} {:>6} {:>6} {:>6} {:>6} {:>5} {:>6} {:>6} {:>5} {:>6} {:>6}",
        "call", "Hz", "Hzband", "1°sing", "1°band", "n.CQ", "%band", "%lane", "drop", "%chiam", "%verif"
    )
    .unwrap();
    // frequenza mediana del canale della banda su cui compare un nominativo
    let hz_band = |call: &str| {
        let mut v: Vec<f32> = radar.iter().filter_map(|r| r.get(call).copied()).collect();
        v.sort_by(f32::total_cmp);
        v.get(v.len() / 2).copied().unwrap_or(0.0)
    };
    let mut found_by_band = 0;
    let (mut sum_pct, mut n_pct) = (0.0, 0);
    let mut firsts: Vec<f64> = Vec::new();
    let mut tot_drops = 0;
    for (call, (f, times)) in &single {
        let mut times = times.clone();
        times.sort_by(f64::total_cmp);
        let first = times[0];
        // attivo: fra due ricorrenze distanti meno di 180 s, e 60 s dopo ognuna
        let active = |t: f64| times.iter().any(|&a| t >= a && t <= a + 60.0) || times.windows(2).any(|w| w[1] - w[0] < 180.0 && t >= w[0] && t <= w[1]);
        let first_band = snaps.iter().zip(&radar).find(|(_, r)| r.contains_key(call)).map(|(s, _)| s.t);
        if let Some(fb) = first_band {
            found_by_band += 1;
            firsts.push((fb - first).max(0.0));
        }
        let (mut n, mut pres, mut lane, mut drops, mut heard, mut verif) = (0, 0, 0, 0, 0, 0);
        let mut was = false;
        for (s, r) in snaps.iter().zip(&radar) {
            if !active(s.t) {
                was = false;
                continue;
            }
            n += 1;
            let p = r.contains_key(call);
            if p {
                pres += 1;
            } else if was {
                drops += 1;
            }
            was = p;
            if s.lanes.iter().any(|l| (l - f).abs() < 60.0) {
                lane += 1;
            }
            if s.calls.iter().any(|(c, _)| c == call) {
                heard += 1;
            }
            if s.calls.iter().any(|(c, b)| c == call && b.verified) {
                verif += 1;
            }
        }
        let pct = |a: i32| if n > 0 { 100.0 * f64::from(a) / f64::from(n) } else { 0.0 };
        sum_pct += pct(pres);
        n_pct += 1;
        tot_drops += drops;
        writeln!(
            rep,
            "{:<10} {:>6.0} {:>6.0} {:>6} {:>6} {:>5} {:>5.0}% {:>5.0}% {:>5} {:>5.0}% {:>5.0}%",
            call,
            f,
            hz_band(call),
            mmss(first),
            first_band.map(mmss).unwrap_or_else(|| "-".into()),
            times.len(),
            pct(pres),
            pct(lane),
            drops,
            pct(heard),
            pct(verif)
        )
        .unwrap();
    }
    let band_all: HashSet<&String> = radar.iter().flat_map(|r| r.keys()).collect();
    let mut extra: Vec<&String> = band_all.iter().copied().filter(|c| !single.contains_key(c.as_str())).collect();
    extra.sort();
    let mean_pct = if n_pct > 0 { sum_pct / f64::from(n_pct) } else { 0.0 };
    firsts.sort_by(f64::total_cmp);
    let median = firsts.get(firsts.len() / 2).copied();
    let mean_listed = radar.iter().map(|r| r.len()).sum::<usize>() as f64 / radar.len().max(1) as f64;
    let mean_lanes = snaps.iter().map(|s| s.lanes.len()).sum::<usize>() as f64 / snaps.len().max(1) as f64;
    writeln!(rep).unwrap();
    writeln!(rep, "chiamanti CQ dal canale singolo: {}", single.len()).unwrap();
    writeln!(rep, "  di questi comparsi almeno una volta nell'elenco della banda: {found_by_band}").unwrap();
    writeln!(rep, "  presenza media nell'elenco mentre sono attivi: {mean_pct:.0}%").unwrap();
    writeln!(rep, "stazioni nell'elenco della banda (CQ verificati): {}", band_all.len()).unwrap();
    let extra_hz: Vec<String> = extra.iter().map(|c| format!("{c}@{:.0}", hz_band(c))).collect();
    writeln!(rep, "  non trovate dal canale singolo ({}): {:?}", extra.len(), extra_hz).unwrap();
    writeln!(rep, "media per istantanea: {mean_listed:.1} stazioni nell'elenco, {mean_lanes:.1} canali aperti").unwrap();
    writeln!(
        rep,
        "RIEPILOGO {mode}: trovati {found_by_band}/{} | presenza {mean_pct:.0}% | ritardo mediano {} | interruzioni {tot_drops} | solo banda {}",
        single.len(),
        median.map(|m| format!("{m:.0} s")).unwrap_or_else(|| "-".into()),
        extra.len()
    )
    .unwrap();
    println!("\n{rep}");
    let out = format!("{path}.bigscan-{mode}.txt");
    match std::fs::write(&out, &rep) {
        Ok(()) => eprintln!("tabella scritta in {out}"),
        Err(e) => eprintln!("tabella non scritta in {out}: {e}"),
    }
}
