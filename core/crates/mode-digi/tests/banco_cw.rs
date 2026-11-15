//! Banco del ricevitore CW misurato a livello di carattere: Morse generato qui con un testo da contest noto (seme fisso), scenari
//! difficili e misure confrontabili prima e dopo ogni modifica. Descrizione completa in `docs/banco-cw.md`.
//!
//! Ogni scenario passa sia al canale singolo (`CwDemodulator`, tono noto) sia a BigScan CW (`CwBandDecoder` su tutta la banda, si
//! raccoglie il testo dei canali entro 25 Hz dal tono). Misure: caratteri giusti, sbagliati, mancanti e inseriti (allineamento di
//! Levenshtein), CER, nominativi esatti e sbagliati, caratteri falsi al minuto sul solo rumore, latenza (mediana e 95° percentile fra la
//! fine del carattere trasmesso e la sua stampa), millisecondi di calcolo per secondo d'audio.
//!
//! - `banco_cw` (lento, `#[ignore]`): la tabella completa; `CW_BANCO_SEMI` semi per scenario (di base 3), `CW_RIGHE` filtra gli
//!   scenari per nome (pezzi separati da virgole), `CW_BIGSCAN=0` salta BigScan, `CW_DEBUG=1` stampa mandato e ricevuto;
//! - tre prove veloci: l'allineamento, un testo pulito e un poco di rumore.
//!
//! `CARGO_BUILD_JOBS=2 nice cargo test --release -j 2 -p mode-digi --test banco_cw -- --ignored --nocapture --test-threads=1`

mod common;

use common::Rng;
use mode_digi::cw::CwDemodulator;
use mode_digi::cw_chain::{ChainParams, CwChain, CwChainBand};
use mode_digi::cw_hybrid::CwHybrid;
use std::sync::atomic::{AtomicU64, Ordering};
use mode_digi::skim::CwBandDecoder;
use std::f64::consts::PI;
use std::time::Instant;

const RATE: f64 = 12_000.0;
const TONO: f64 = 700.0;
/// Deviazione del rumore bianco a 12 kHz; le ampiezze dei segnali seguono il loro S/N in 500 Hz.
const SIGMA: f64 = 0.05;
/// Blocchi d'audio come nel servizio (100 ms).
const BLOCCO: usize = 1_200;

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

// ---------------------------------------------------------------- testo

const PREFISSI: [&str; 20] = ["K", "W", "N", "DL", "G", "F", "EA", "OH", "JA", "VK", "SP", "OK", "HA", "YO", "LZ", "ON", "PA", "OE", "S5", "UA"];

fn nominativo(rng: &mut Rng) -> String {
    let p = PREFISSI[(rng.next_u64() % PREFISSI.len() as u64) as usize];
    let d = rng.next_u64() % 10;
    let n = 1 + (rng.next_u64() % 3) as usize;
    let s: String = (0..n).map(|_| char::from(b'A' + (rng.next_u64() % 26) as u8)).collect();
    format!("{p}{d}{s}")
}

/// Un pezzo di QSO da contest: (testo, nominativi presenti).
fn frase(rng: &mut Rng) -> (String, Vec<String>) {
    let me = if rng.uniform() < 0.3 { "IU3ABC".to_string() } else { nominativo(rng) };
    let other = nominativo(rng);
    let nr = 1 + rng.next_u64() % 999;
    match rng.next_u64() % 6 {
        0 => (format!("CQ TEST {me} {me}"), vec![me]),
        1 => (format!("{other} 5NN {nr:03}"), vec![other]),
        2 => (format!("TU {me} TEST"), vec![me]),
        3 => (format!("{other} {other}"), vec![other]),
        4 => (format!("R 5NN {nr} TU"), vec![]),
        _ => (format!("CQ {me} {me} IO91WM K"), vec![me]),
    }
}

/// Testo da contest di almeno `chars` caratteri.
fn testo_contest(rng: &mut Rng, chars: usize) -> (String, Vec<String>) {
    let (mut s, mut calls) = (String::new(), Vec::new());
    while s.len() < chars {
        let (f, c) = frase(rng);
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&f);
        calls.extend(c);
    }
    (s, calls)
}

// ---------------------------------------------------------------- manipolazione

/// Come si manipola: irregolarita' relativa di toni e pause e allungamento delle pause.
#[derive(Clone, Copy)]
struct Mano {
    jitter: f64,
    spazi: f64,
}

const KEYER: Mano = Mano { jitter: 0.03, spazi: 0.0 };

/// Un evento della manipolazione: acceso o spento e durata (s); per ogni carattere il momento (s, dall'inizio) in cui finisce.
struct Chiavi {
    seq: Vec<(bool, f64)>,
    fine_car: Vec<f64>,
}

/// La manipolazione di `text` alla velocita' `wpm(i)` (i = indice del carattere).
fn chiavi(text: &str, wpm: impl Fn(usize) -> f64, m: Mano, rng: &mut Rng) -> Chiavi {
    let mut seq = Vec::new();
    let mut fine_car = Vec::new();
    let mut t = 0.0;
    let j = |x: f64, rng: &mut Rng, extra: f64| (x * (1.0 + m.jitter * rng.gauss()) * (1.0 + extra * rng.uniform())).max(0.35 * x);
    for (i, c) in text.bytes().enumerate() {
        let unit = 1.2 / wpm(i);
        if c == b' ' {
            // dopo la pausa fra i caratteri (3) altre 4 unita'
            let d = j(4.0 * unit, rng, m.spazi);
            seq.push((false, d));
            t += d;
            fine_car.push(t);
            continue;
        }
        let Some(p) = morse(c) else {
            fine_car.push(t);
            continue;
        };
        for (k, e) in p.bytes().enumerate() {
            if k > 0 {
                let d = j(unit, rng, 0.0);
                seq.push((false, d));
                t += d;
            }
            let d = j(if e == b'-' { 3.0 } else { 1.0 } * unit, rng, 0.0);
            seq.push((true, d));
            t += d;
        }
        fine_car.push(t);
        let d = j(3.0 * unit, rng, m.spazi);
        seq.push((false, d));
        t += d;
    }
    Chiavi { seq, fine_car }
}

/// Suona `seq` da `start` (campioni) in `band` con ampiezza `amp(t)`, fronti a coseno di 4 ms.
fn suona(band: &mut [f32], hz: f64, start: f64, seq: &[(bool, f64)], amp: &dyn Fn(usize) -> f64) {
    let ramp = 0.004 * RATE;
    let mut t_end = start;
    for &(on, secs) in seq {
        let s0 = t_end.round() as usize;
        t_end += secs * RATE;
        if !on {
            continue;
        }
        let len = (t_end.round() as usize).saturating_sub(s0);
        for i in 0..len {
            let n = s0 + i;
            if n >= band.len() {
                return;
            }
            let edge = i.min(len - 1 - i) as f64;
            let w = if edge < ramp { 0.5 * (1.0 - (PI * edge / ramp).cos()) } else { 1.0 };
            band[n] += (amp(n) * w * (2.0 * PI * hz * n as f64 / RATE).sin()) as f32;
        }
    }
}

/// Ampiezza di una sinusoide con S/N `snr_db` in 500 Hz contro il rumore `SIGMA` a 12 kHz: A²/2 = S/N · σ² · 500 / 6000.
fn amp_per_snr(snr_db: f64) -> f64 {
    SIGMA * (2.0 * 500.0 / (RATE / 2.0) * 10f64.powf(snr_db / 10.0)).sqrt()
}

/// Modulo di un percorso di Rayleigh (potenza media 1) con banda Doppler `hz`.
fn rayleigh(len: usize, hz: f64, rng: &mut Rng) -> Vec<f64> {
    common::percorso_rayleigh(len, hz, rng).into_iter().map(|(a, b)| (a * a + b * b).sqrt()).collect()
}

// ---------------------------------------------------------------- scenari

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tipo {
    Pulito,
    Salto,
    Manuale,
    QsbLento,
    QsbVeloce,
    Crepitii,
    Vicina,
    Silenzi,
    Rumore,
}

#[derive(Clone, Copy)]
struct Scenario {
    nome: &'static str,
    tipo: Tipo,
    wpm: f64,
    snr_db: f64,
}

const fn sc(nome: &'static str, tipo: Tipo, wpm: f64, snr_db: f64) -> Scenario {
    Scenario { nome, tipo, wpm, snr_db }
}

const SCENARI: [Scenario; 21] = [
    sc("18wpm +10", Tipo::Pulito, 18.0, 10.0),
    sc("25wpm +10", Tipo::Pulito, 25.0, 10.0),
    sc("32wpm +10", Tipo::Pulito, 32.0, 10.0),
    sc("40wpm +10", Tipo::Pulito, 40.0, 10.0),
    sc("25wpm +3", Tipo::Pulito, 25.0, 3.0),
    sc("25wpm 0", Tipo::Pulito, 25.0, 0.0),
    sc("25wpm -3", Tipo::Pulito, 25.0, -3.0),
    sc("25wpm -6", Tipo::Pulito, 25.0, -6.0),
    sc("40wpm 0", Tipo::Pulito, 40.0, 0.0),
    sc("salto 18-35", Tipo::Salto, 18.0, 6.0),
    sc("manuale +10", Tipo::Manuale, 20.0, 10.0),
    sc("manuale +3", Tipo::Manuale, 20.0, 3.0),
    sc("qsb lento", Tipo::QsbLento, 25.0, 6.0),
    sc("qsb veloce", Tipo::QsbVeloce, 25.0, 6.0),
    sc("crepitii", Tipo::Crepitii, 25.0, 6.0),
    sc("vicina +20", Tipo::Vicina, 25.0, 3.0),
    sc("silenzi", Tipo::Silenzi, 25.0, 6.0),
    sc("silenzi 0", Tipo::Silenzi, 25.0, 0.0),
    sc("silenzi 32", Tipo::Silenzi, 32.0, 6.0),
    sc("qsb lento 0", Tipo::QsbLento, 25.0, 0.0),
    sc("rumore 60s", Tipo::Rumore, 25.0, 0.0),
];

/// Un caso pronto: audio, testo mandato (senza spazi doppi), nominativi, fine di ogni carattere (s) e secondi in cui c'e' segnale.
struct Caso {
    audio: Vec<f32>,
    testo: String,
    calls: Vec<String>,
    fine_car: Vec<f64>,
}

fn prepara(s: Scenario, seed: u64) -> Caso {
    prepara_lungo(s, seed, 40.0)
}

/// Come [`prepara`] con circa `secondi` di testo (il solo rumore dura sempre 60 s).
fn prepara_lungo(s: Scenario, seed: u64, secondi: f64) -> Caso {
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    if s.tipo == Tipo::Rumore {
        let mut audio = vec![0.0f32; (60.0 * RATE) as usize];
        common::aggiungi_rumore(&mut audio, SIGMA, &mut rng);
        return Caso { audio, testo: String::new(), calls: Vec::new(), fine_car: Vec::new() };
    }
    let amp0 = amp_per_snr(s.snr_db);
    let chars = (secondi * s.wpm * 50.0 / 60.0 / 8.0) as usize;
    let (testo, calls) = testo_contest(&mut rng, chars.max(40));
    let mano = match s.tipo {
        Tipo::Manuale => Mano { jitter: 0.15 + 0.10 * rng.uniform(), spazi: 0.6 },
        _ => KEYER,
    };
    let start = 1.5 * RATE;
    let mut fine_car = Vec::new();
    let mut segmenti: Vec<(f64, Vec<(bool, f64)>)> = Vec::new();
    match s.tipo {
        Tipo::Silenzi => {
            // trasmissioni separate da silenzi di 4-8 s, una frase per volta
            let mut t = start;
            let parti: Vec<&str> = split_frasi(&testo);
            for (k, p) in parti.iter().enumerate() {
                if k > 0 {
                    // lo spazio fra due trasmissioni conta come un carattere che finisce all'inizio della successiva
                    t += (4.0 + 4.0 * rng.uniform()) * RATE;
                    fine_car.push(t / RATE);
                }
                let ch = chiavi(p, |_| s.wpm, mano, &mut rng);
                fine_car.extend(ch.fine_car.iter().map(|f| t / RATE + f));
                let d: f64 = ch.seq.iter().map(|x| x.1).sum();
                segmenti.push((t, ch.seq));
                t += d * RATE;
            }
        }
        _ => {
            let n = testo.len();
            let salto = s.tipo == Tipo::Salto;
            let ch = chiavi(&testo, |i| if salto && i >= n / 2 { 35.0 } else { s.wpm }, mano, &mut rng);
            fine_car.extend(ch.fine_car.iter().map(|f| start / RATE + f));
            segmenti.push((start, ch.seq));
        }
    }
    let fine = segmenti.last().map(|(t, q)| t + q.iter().map(|x| x.1).sum::<f64>() * RATE).unwrap_or(start);
    let len = (fine + 3.0 * RATE) as usize;
    let mut audio = vec![0.0f32; len];
    common::aggiungi_rumore(&mut audio, SIGMA, &mut rng);
    let fading: Option<Vec<f64>> = match s.tipo {
        Tipo::QsbLento => Some(rayleigh(len, 0.1, &mut rng)),
        Tipo::QsbVeloce => Some(rayleigh(len, 2.0, &mut rng)),
        _ => None,
    };
    let amp = |n: usize| amp0 * fading.as_ref().map_or(1.0, |f| f[n.min(f.len() - 1)]);
    for (t, seq) in &segmenti {
        suona(&mut audio, TONO, *t, seq, &amp);
    }
    match s.tipo {
        Tipo::Vicina => {
            // un'altra stazione 20 dB piu' forte a +150 Hz, testo e velocita' suoi
            let (altro, _) = testo_contest(&mut rng, 400);
            let ch = chiavi(&altro, |_| 28.0, KEYER, &mut rng);
            let a = amp0 * 10.0;
            suona(&mut audio, TONO + 150.0, 0.3 * RATE, &ch.seq, &|_| a);
        }
        Tipo::Crepitii => {
            // scariche: impulsi di 1-5 ms, da 5 a 30 volte il rumore, circa 4 al secondo
            let mut n = 0usize;
            loop {
                n += (RATE * (0.05 + 0.45 * rng.uniform())) as usize;
                if n >= len {
                    break;
                }
                let dur = (RATE * (0.001 + 0.004 * rng.uniform())) as usize;
                let a = SIGMA * (5.0 + 25.0 * rng.uniform());
                for k in 0..dur.min(len - n) {
                    audio[n + k] += (a * rng.gauss() * (-(k as f64) / dur as f64 * 3.0).exp()) as f32;
                }
            }
        }
        _ => {}
    }
    Caso { audio, testo, calls, fine_car }
}

/// Le frasi del testo (ogni frase e' una trasmissione): si spezza ogni 3-6 parole, sempre allo stesso modo.
fn split_frasi(testo: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut inizio = 0;
    let mut parole = 0;
    for (i, c) in testo.char_indices() {
        if c == ' ' {
            parole += 1;
            if parole >= 3 + (inizio % 4) {
                out.push(&testo[inizio..i]);
                inizio = i + 1;
                parole = 0;
            }
        }
    }
    if inizio < testo.len() {
        out.push(&testo[inizio..]);
    }
    out
}

// ---------------------------------------------------------------- misure

/// Il testo ricevuto normalizzato: maiuscole, segni di procedura come un carattere `*`, spazi singoli; ogni carattere con il momento
/// (s) in cui e' stato stampato.
fn normalizza(raw: &[(u8, f64)]) -> Vec<(u8, f64)> {
    let mut out: Vec<(u8, f64)> = Vec::new();
    let mut in_pro = false;
    for &(c, t) in raw {
        let c = c.to_ascii_uppercase();
        if c == b'<' {
            in_pro = true;
            out.push((b'*', t));
            continue;
        }
        if in_pro {
            if c == b'>' {
                in_pro = false;
            }
            continue;
        }
        let c = if c.is_ascii_whitespace() { b' ' } else { c };
        if c == b' ' && out.last().is_none_or(|l| l.0 == b' ') {
            continue;
        }
        out.push((c, t));
    }
    while out.last().is_some_and(|l| l.0 == b' ') {
        out.pop();
    }
    out
}

/// Allineamento di Levenshtein: (giusti, sbagliati, mancanti, inseriti) e, per ogni carattere mandato giusto, l'indice del ricevuto.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
struct Conta {
    giusti: usize,
    sbagliati: usize,
    mancanti: usize,
    inseriti: usize,
}

fn allinea(sent: &[u8], got: &[u8]) -> (Conta, Vec<(usize, usize)>) {
    let (n, m) = (sent.len(), got.len());
    let mut d = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in 0..=n {
        d[at(i, 0)] = i as u32;
    }
    for j in 0..=m {
        d[at(0, j)] = j as u32;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = d[at(i - 1, j - 1)] + u32::from(sent[i - 1] != got[j - 1]);
            d[at(i, j)] = sub.min(d[at(i - 1, j)] + 1).min(d[at(i, j - 1)] + 1);
        }
    }
    let (mut i, mut j) = (n, m);
    let mut c = Conta::default();
    let mut pairs = Vec::new();
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && d[at(i, j)] == d[at(i - 1, j - 1)] + u32::from(sent[i - 1] != got[j - 1]) {
            if sent[i - 1] == got[j - 1] {
                c.giusti += 1;
                pairs.push((i - 1, j - 1));
            } else {
                c.sbagliati += 1;
            }
            i -= 1;
            j -= 1;
        } else if i > 0 && d[at(i, j)] == d[at(i - 1, j)] + 1 {
            c.mancanti += 1;
            i -= 1;
        } else {
            c.inseriti += 1;
            j -= 1;
        }
    }
    (c, pairs)
}

/// Sembra un nominativo: 4-8 caratteri con lettere e almeno una cifra non in fondo (un locatore come IO91WM no).
fn sembra_nominativo(w: &str) -> bool {
    let b = w.as_bytes();
    let locatore = b.len() == 6 && b[..2].iter().all(u8::is_ascii_alphabetic) && b[2..4].iter().all(u8::is_ascii_digit) && b[4..].iter().all(u8::is_ascii_alphabetic);
    !locatore
        && (4..=8).contains(&b.len())
        && b.iter().all(u8::is_ascii_alphanumeric)
        && b.iter().filter(|c| c.is_ascii_alphabetic()).count() >= 2
        && b[..b.len() - 1].iter().any(u8::is_ascii_digit)
        && b.last().is_some_and(u8::is_ascii_alphabetic)
}

/// Risultato di un caso su un ricevitore.
#[derive(Default, Clone)]
struct Esito {
    conta: Conta,
    mandati: usize,
    calls_esatti: usize,
    calls_tot: usize,
    calls_sbagliati: usize,
    /// Caratteri non spazio stampati (per il solo rumore).
    stampati: usize,
    /// Errori e caratteri mandati senza contare gli spazi (quanto pesano le parole spezzate o unite).
    err_ns: usize,
    mandati_ns: usize,
    latenze: Vec<f64>,
    secondi: f64,
    cpu_s: f64,
}

fn valuta(caso: &Caso, raw: &[(u8, f64)], cpu_s: f64) -> Esito {
    let got = normalizza(raw);
    let got_b: Vec<u8> = got.iter().map(|x| x.0).collect();
    let sent = caso.testo.as_bytes();
    let (conta, pairs) = allinea(sent, &got_b);
    let latenze = pairs.iter().filter(|(i, _)| sent[*i] != b' ').filter_map(|&(i, j)| caso.fine_car.get(i).map(|f| got[j].1 - f)).collect();
    let words: Vec<&str> = std::str::from_utf8(&got_b).unwrap_or("").split(' ').collect();
    let mut esatti = 0;
    let mut cercati: Vec<&str> = caso.calls.iter().map(String::as_str).collect();
    // ogni volta che un nominativo e' mandato conta una volta: esatto se la parola compare (consumata nell'ordine)
    let mut pos = 0;
    for c in &cercati {
        if let Some(k) = words[pos..].iter().position(|w| w == c) {
            esatti += 1;
            pos += k + 1;
        }
    }
    let ns = |v: &[u8]| v.iter().copied().filter(|c| *c != b' ').collect::<Vec<u8>>();
    let (sent_ns, got_ns) = (ns(sent), ns(&got_b));
    let err_ns = common::levenshtein(&sent_ns, &got_ns);
    cercati.sort_unstable();
    cercati.dedup();
    let sbagliati = words.iter().filter(|w| sembra_nominativo(w) && cercati.binary_search(w).is_err()).count();
    Esito {
        conta,
        mandati: sent.len(),
        calls_esatti: esatti,
        calls_tot: caso.calls.len(),
        calls_sbagliati: sbagliati,
        stampati: got_ns.len(),
        err_ns,
        mandati_ns: sent_ns.len(),
        latenze,
        secondi: caso.audio.len() as f64 / RATE,
        cpu_s,
    }
}

/// Quale ricevitore si misura: `CW_RX=catena` il ricevitore a catena (`cw_chain`), altrimenti quello attuale.
fn catena() -> bool {
    std::env::var("CW_RX").is_ok_and(|v| v == "catena")
}

/// `CW_RX=ibrido`: la catena per il testo provvisorio, il ricevitore attuale per il definitivo (latenza = primo momento in cui il
/// carattere giusto e' sullo schermo).
fn ibrido() -> bool {
    std::env::var("CW_RX").is_ok_and(|v| v == "ibrido")
}

static PROVVISORI: AtomicU64 = AtomicU64::new(0);
static CONFERMATI: AtomicU64 = AtomicU64::new(0);

/// I parametri della catena, con le modifiche di `CW_PARAM` (`nome=valore,nome=valore`).
fn parametri() -> ChainParams {
    let mut p = ChainParams::default();
    let bad = p.apply_list(&std::env::var("CW_PARAM").unwrap_or_default());
    assert!(bad.is_empty(), "parametri sconosciuti: {bad:?}");
    p
}

/// Il canale singolo, con le impostazioni di base del servizio, al tono giusto.
fn canale(caso: &Caso) -> Esito {
    let mut raw = Vec::new();
    let t0 = Instant::now();
    if ibrido() {
        let mut rx = CwHybrid::with_params(CwDemodulator::new(TONO as f32, 20.0), 20.0, &parametri());
        for block in caso.audio.chunks(BLOCCO) {
            raw.extend(rx.process(block).fixed.into_iter().map(|s| (s.ch, s.at_s)));
        }
        let st = rx.stats();
        PROVVISORI.fetch_add(st.provisional, Ordering::Relaxed);
        CONFERMATI.fetch_add(st.confirmed, Ordering::Relaxed);
    } else if catena() {
        let mut rx = CwChain::with_params(TONO as f32, 20.0, &parametri());
        for (k, block) in caso.audio.chunks(BLOCCO).enumerate() {
            let t = ((k + 1) * BLOCCO) as f64 / RATE;
            raw.extend(rx.process(block).into_iter().map(|c| (c, t)));
        }
    } else {
        let mut rx = CwDemodulator::new(TONO as f32, 20.0);
        for (k, block) in caso.audio.chunks(BLOCCO).enumerate() {
            let t = ((k + 1) * BLOCCO) as f64 / RATE;
            raw.extend(rx.process(block).into_iter().map(|c| (c, t)));
            if std::env::var_os("CW_DEBUG_WPM").is_some() && k % 20 == 19 {
                print!("{:.0}/{:.0} ", rx.wpm(), rx.snr_db());
            }
        }
    }
    let cpu = t0.elapsed().as_secs_f64();
    if std::env::var_os("CW_DEBUG").is_some() {
        println!("  ricevuto: {}", String::from_utf8_lossy(&raw.iter().map(|x| x.0).collect::<Vec<u8>>()));
    }
    valuta(caso, &raw, cpu)
}

/// Quanto di `new` e' nuovo rispetto a `old` (il testo dei canali e' una finestra che scorre: si toglie in testa e si aggiunge in coda).
fn aggiunto<'a>(old: &str, new: &'a str) -> &'a str {
    if let Some(rest) = new.strip_prefix(old) {
        return rest;
    }
    let ob = old.as_bytes();
    let nb = new.as_bytes();
    for m in (1..=nb.len().min(ob.len())).rev() {
        if ob.ends_with(&nb[..m]) {
            return &new[m..];
        }
    }
    new
}

/// BigScan CW su tutta la banda: il testo dei canali entro 25 Hz dal tono (sul rumore di tutti i canali).
fn bigscan(caso: &Caso, tutti: bool) -> Esito {
    if catena() {
        return bigscan_catena(caso, tutti);
    }
    let mut dec = CwBandDecoder::new(150.0, 4_000.0, 40);
    let mut last: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut raw = Vec::new();
    let mut cpu = 0.0;
    for (k, block) in caso.audio.chunks(BLOCCO).enumerate() {
        let t0 = Instant::now();
        dec.process(block);
        cpu += t0.elapsed().as_secs_f64();
        let t = ((k + 1) * BLOCCO) as f64 / RATE;
        for ch in dec.channels() {
            let old = last.entry(ch.id).or_default();
            let add = aggiunto(old, &ch.text).to_string();
            *old = ch.text.clone();
            if tutti || (f64::from(ch.freq_hz) - TONO).abs() <= 25.0 {
                raw.extend(add.bytes().map(|c| (c, t)));
            }
        }
    }
    valuta(caso, &raw, cpu)
}

/// BigScan con la catena ([`CwChainBand`]), raccolto come [`bigscan`].
fn bigscan_catena(caso: &Caso, tutti: bool) -> Esito {
    let mut dec = CwChainBand::with_params(150.0, 4_000.0, 40, &parametri());
    let mut last: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut raw = Vec::new();
    let mut cpu = 0.0;
    for (k, block) in caso.audio.chunks(BLOCCO).enumerate() {
        let t0 = Instant::now();
        dec.process(block);
        cpu += t0.elapsed().as_secs_f64();
        let t = ((k + 1) * BLOCCO) as f64 / RATE;
        for ch in dec.channels() {
            let old = last.entry(ch.id).or_default();
            let add = aggiunto(old, &ch.text).to_string();
            *old = ch.text.clone();
            if tutti || (f64::from(ch.freq_hz) - TONO).abs() <= 25.0 {
                if std::env::var_os("CW_DEBUG").is_some() && !add.is_empty() {
                    println!("  {t:.1}s canale {} {:.0} Hz: {add:?}", ch.id, ch.freq_hz);
                }
                raw.extend(add.bytes().map(|c| (c, t)));
            }
        }
    }
    if std::env::var_os("CW_DEBUG").is_some() {
        for ch in dec.channels() {
            println!("  canale {} {:.0} Hz: {}", ch.id, ch.freq_hz, ch.text);
        }
    }
    valuta(caso, &raw, cpu)
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}

#[derive(Default)]
struct Somma {
    e: Esito,
}

impl Somma {
    fn add(&mut self, x: &Esito) {
        let e = &mut self.e;
        e.conta.giusti += x.conta.giusti;
        e.conta.sbagliati += x.conta.sbagliati;
        e.conta.mancanti += x.conta.mancanti;
        e.conta.inseriti += x.conta.inseriti;
        e.mandati += x.mandati;
        e.calls_esatti += x.calls_esatti;
        e.calls_tot += x.calls_tot;
        e.calls_sbagliati += x.calls_sbagliati;
        e.stampati += x.stampati;
        e.err_ns += x.err_ns;
        e.mandati_ns += x.mandati_ns;
        e.latenze.extend_from_slice(&x.latenze);
        e.secondi += x.secondi;
        e.cpu_s += x.cpu_s;
    }

    fn riga(&mut self, chi: &str, nome: &str, rumore: bool) -> String {
        let e = &mut self.e;
        let c = e.conta;
        let cpu = 1000.0 * e.cpu_s / e.secondi.max(1e-9);
        if rumore {
            return format!("{chi:<4} {nome:<13} falsi {:.2}/min ({} in {:.0} min)   cpu {cpu:.1} ms/s", e.stampati as f64 * 60.0 / e.secondi, e.stampati, e.secondi / 60.0);
        }
        let cer = 100.0 * (c.sbagliati + c.mancanti + c.inseriti) as f64 / e.mandati.max(1) as f64;
        let cer_ns = 100.0 * e.err_ns as f64 / e.mandati_ns.max(1) as f64;
        let (p50, p95) = (percentile(&mut e.latenze, 0.5), percentile(&mut e.latenze, 0.95));
        format!(
            "{chi:<4} {nome:<13} CER {cer:5.1}% ns {cer_ns:5.1}%  giusti {:5} sbagl {:4} manc {:4} ins {:4}  call {:3}/{:3} falsi {:3}  lat {:.2}/{:.2} s  cpu {cpu:.1} ms/s",
            c.giusti, c.sbagliati, c.mancanti, c.inseriti, e.calls_esatti, e.calls_tot, e.calls_sbagliati, p50, p95
        )
    }
}

fn scelti() -> Vec<Scenario> {
    let scelta = std::env::var("CW_RIGHE").unwrap_or_default().to_lowercase();
    let pezzi: Vec<&str> = scelta.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
    SCENARI.iter().copied().filter(|s| pezzi.is_empty() || pezzi.iter().any(|p| s.nome.to_lowercase().contains(p))).collect()
}

#[test]
#[ignore]
fn banco_cw() {
    let semi: u64 = std::env::var("CW_BANCO_SEMI").ok().and_then(|s| s.parse().ok()).unwrap_or(3);
    let con_bigscan = std::env::var("CW_BIGSCAN").map_or(true, |v| v != "0");
    let debug = std::env::var_os("CW_DEBUG").is_some();
    let (mut tot1, mut tot2) = (Somma::default(), Somma::default());
    let mut cer_somma = (0.0, 0.0, 0usize);
    println!();
    for s in scelti() {
        let rumore = s.tipo == Tipo::Rumore;
        let (mut a, mut b) = (Somma::default(), Somma::default());
        for seed in 0..semi {
            let caso = prepara(s, 7_000 + seed);
            let e1 = canale(&caso);
            if debug {
                println!("  mandato : {}", caso.testo);
            }
            a.add(&e1);
            if !rumore {
                tot1.add(&e1);
            }
            if con_bigscan {
                let e2 = bigscan(&caso, rumore);
                b.add(&e2);
                if !rumore {
                    tot2.add(&e2);
                }
            }
        }
        if !rumore {
            let cer = |x: &Esito| 100.0 * (x.conta.sbagliati + x.conta.mancanti + x.conta.inseriti) as f64 / x.mandati.max(1) as f64;
            cer_somma.0 += cer(&a.e);
            cer_somma.1 += cer(&b.e);
            cer_somma.2 += 1;
        }
        println!("{}", a.riga("ch", s.nome, rumore));
        if con_bigscan {
            println!("{}", b.riga("big", s.nome, rumore));
        }
    }
    let n = cer_somma.2.max(1) as f64;
    println!("{}", tot1.riga("ch", "TOTALE", false));
    if con_bigscan {
        println!("{}", tot2.riga("big", "TOTALE", false));
    }
    println!("RIEPILOGO CER medio per scenario: canale {:.2}% bigscan {:.2}%", cer_somma.0 / n, cer_somma.1 / n);
    let (pv, cf) = (PROVVISORI.load(Ordering::Relaxed), CONFERMATI.load(Ordering::Relaxed));
    if pv > 0 {
        println!("PROVVISORI {pv} confermati {cf} corretti {:.1}%", 100.0 * (pv.saturating_sub(cf)) as f64 / pv as f64);
    }
}

// ---------------------------------------------------------------- prove veloci

#[test]
fn allineamento_conta_i_quattro_tipi() {
    let (c, pairs) = allinea(b"CQ TEST", b"CQ TXST");
    assert_eq!(c, Conta { giusti: 6, sbagliati: 1, mancanti: 0, inseriti: 0 });
    assert_eq!(pairs.len(), 6);
    let (c, _) = allinea(b"IU3ABC", b"IU3AABC");
    assert_eq!((c.giusti, c.inseriti), (6, 1));
    let (c, _) = allinea(b"IU3ABC", b"I3ABC");
    assert_eq!((c.giusti, c.mancanti), (5, 1));
    assert!(sembra_nominativo("IU3ABC") && sembra_nominativo("K1XY") && !sembra_nominativo("5NN") && !sembra_nominativo("TEST") && !sembra_nominativo("IO91WM"));
    assert_eq!(aggiunto("ABCD", "CDEF"), "EF");
    assert_eq!(aggiunto("AB", "ABC"), "C");
}

#[test]
fn testo_pulito_si_legge() {
    let caso = prepara_lungo(sc("25wpm +10", Tipo::Pulito, 25.0, 10.0), 1, 10.0);
    let e = canale(&caso);
    let cer = (e.conta.sbagliati + e.conta.mancanti + e.conta.inseriti) as f64 / e.mandati as f64;
    assert!(cer < 0.1, "CER {cer} su {:?}", caso.testo);
}

#[test]
fn poco_rumore_quasi_niente_falsi() {
    let mut caso = prepara(sc("rumore", Tipo::Rumore, 25.0, 0.0), 2);
    caso.audio.truncate((15.0 * RATE) as usize);
    let e = canale(&caso);
    assert!(e.stampati <= 3, "{} caratteri falsi in 15 s di rumore", e.stampati);
}
