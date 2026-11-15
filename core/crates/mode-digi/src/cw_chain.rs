//! Ricevitore CW a catena: un secondo ricevitore completo, separato da quello di [`crate::cw`] / [`crate::cw_bayes`], costruito come
//! una catena di stadi indipendenti e misurato sullo stesso banco (`tests/banco_cw.rs`, variabile `CW_RX=catena`).
//!
//! Gli stadi, nell'ordine:
//! 1. **spettro a cadenza fissa**: 93,75 spettri al secondo (passo di 128 campioni a 12 kHz, 1024 punti, righe di 11,72 Hz), finestra
//!    a quattro coseni per un passa-basso a sinc da 25 Hz; attorno al tono il **confronto di forma** su ±10 righe: ampiezza dalla riga
//!    centrale, fondo dalle due spalle simmetriche (righe 4-8 da ogni lato, con i prodotti delle coppie), residuo = ampiezza - fondo;
//!    tutto normalizzato sul rumore della banda (mediana dello spettro);
//! 2. **inviluppo**: mediana di 3 del residuo, media lunga (71 passi) con cancello a isteresi (presenza della stazione), massimo (17) e
//!    media corta (9) per l'inviluppo superiore, fondo dinamico col minimo scorrevole (111);
//! 3. **probabilita' continua di tasto abbassato**: somma di due tanh (ampiezza e residuo meno il fondo), quasi zero a cancello chiuso;
//!    piu' un'indicazione di qualita' (media lunga bassa);
//! 4. **decodifica di sequenza a durate esplicite**: età del tono (20) e del silenzio (30) con code, probabilita' di fine dalle densita'
//!    delle durate, percorso migliore con risalita a blocchi di 25 passi su una storia di 50;
//! 5. **cinque classi di durata lognormali** (punto, linea, pausa breve, due pause lunghe con code uniformi); avvio sulle ipotesi dei
//!    primi 14 elementi, poi adattamento correlato di tipo Kalman delle cinque medie (varianze diverse in apprendimento e in
//!    inseguimento); WPM da punto, linea e pausa breve;
//! 6. **conversione Morse** con tentativo di divisione in due caratteri per i codici sconosciuti;
//! 7. **segmentazione delle parole** con un grafo lessicale pesato (parole del traffico, rapporti, CQ/DE/TEST, QRL, schemi generici,
//!    struttura del nominativo) e percorso migliore: decide solo i confini, mai le lettere; consolidamento dei nominativi ripetuti;
//! 8. per BigScan ([`CwChainBand`]): accumulatore per riga, contrasto locale a ±4 righe, massimi stretti con maschera attorno ai canali
//!    gia' aperti, ciclo di vita dei canali (apprendimento, attivo, inattivo).
//!
//! I valori di partenza vengono dalla descrizione del modello; quelli in [`ChainParams::default`] sono tarati sul banco.

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};
use std::collections::VecDeque;
use std::f64::consts::PI;
use std::sync::Arc;

/// Campioni al secondo dell'audio in ingresso.
const RATE: f64 = 12_000.0;
/// Passo fra due spettri (campioni): 93,75 spettri al secondo.
pub const HOP: usize = 128;
/// Punti dello spettro: otto passi.
pub const NFFT: usize = 8 * HOP;
/// Larghezza di una riga (Hz).
pub const BIN_HZ: f64 = RATE / NFFT as f64;
/// Spettri al secondo.
pub const STEPS_PER_S: f64 = RATE / HOP as f64;
/// Righe da ogni lato nel confronto di forma.
const SHAPE_HALF: usize = 10;
/// Età massime del tono e del silenzio nella decodifica di sequenza (l'ultima e' la coda).
const ON_AGES: usize = 20;
const OFF_AGES: usize = 30;
/// Blocco di decisione e storia della risalita (passi).
const DECIDE: usize = 25;
const HISTORY: usize = 50;
/// Elementi raccolti per l'avvio.
const STARTUP_ELEMENTS: usize = 14;
/// Punteggio di un percorso impossibile.
const NEG: f64 = -1.0e4;

// ---------------------------------------------------------------- parametri

/// I parametri della catena: valori di partenza dalla descrizione del modello, poi tarati sul banco.
#[derive(Clone, Debug)]
pub struct ChainParams {
    /// Taglio del passa-basso della finestra (Hz).
    pub window_fc: f64,
    /// Cancello di presenza sulla media lunga del residuo: apertura e tenuta.
    pub gate_on: f32,
    pub gate_off: f32,
    /// Lunghezze di media lunga, massimo, media corta e minimo dell'inviluppo (passi).
    pub len_long: usize,
    pub len_max: usize,
    pub len_short: usize,
    pub len_min: usize,
    /// Fondo massimo e inviluppo superiore: fondo = min(fondo_max, base + pendenza * max(0, inviluppo - base)).
    pub floor_max: f32,
    pub env_base: f32,
    pub env_slope: f32,
    /// Il fondo scende solo sui picchi oltre questa frazione di (fondo massimo + media lunga).
    pub peak_k: f32,
    /// Pavimento delle due grandezze nelle tanh.
    pub tanh_floor: f32,
    /// Pendenza delle tanh.
    pub tanh_gain: f32,
    /// Probabilita' a cancello chiuso.
    pub p_closed: f32,
    /// Emissioni: 0 = due tanh, 1 = verosimiglianza di Rice contro Rayleigh (ibrido).
    pub emit_mode: u8,
    /// Ibrido: peso della verosimiglianza (i passi sono correlati) e S/N minimo creduto (potenza).
    pub rice_weight: f64,
    pub rice_min: f64,
    /// Ibrido: le spalle normalizzano solo oltre questa potenza (riferita al rumore della banda).
    pub side_k: f32,
    /// Ibrido: S/N del tono dall'inviluppo superiore corto invece che dalla media lunga.
    pub a2_short: bool,
    /// Ibrido coerente (`emit_mode` 2): taglio della finestra della banda base, unita' di un pezzo sommato in fase, S/N massimo creduto
    /// per passo, pezzi al massimo in un tono.
    pub iq_fc: f64,
    pub coh_piece_units: f64,
    pub coh_snr_cap: f64,
    pub coh_max_pieces: usize,
    /// Ibrido coerente: ampiezza al quadrato minima creduta per passo.
    pub coh_min: f64,
    /// Ibrido coerente: penalita' per aprire un tono a cancello chiuso (logaritmo; molto grande = mai).
    pub coh_closed: f64,
    /// Peso delle emissioni nella decodifica di sequenza.
    pub emit_weight: f64,
    /// Pesi delle cinque classi di durata.
    pub class_w: [f64; 5],
    /// Peso delle code uniformi delle due pause lunghe.
    pub tail3: f64,
    pub tail4: f64,
    /// Varianze di partenza nel dominio logaritmico, moltiplicate per questo fattore.
    pub r_scale: f64,
    /// Rumore di processo del filtro di Kalman (frazione della covarianza a priori).
    pub q_frac: f64,
    /// Varianza minima delle classi (dominio logaritmico).
    pub r_floor: f64,
    /// Deviazione massima delle classi (dominio logaritmico) moltiplicata per la media: c / mu.
    pub r_cap: f64,
    /// Elementi in apprendimento dopo l'avvio.
    pub learn_elements: usize,
    /// Lettere tenute prima di decidere lo spazio (0 = lo spazio lo decide solo la pausa).
    pub seg_lookahead: usize,
    /// Peso del lessico nella segmentazione.
    pub lex_weight: f64,
    /// Passi di cancello chiuso prima di chiudere la sequenza.
    pub flush_steps: usize,
    /// Elementi oltre i quali un carattere fa rifare l'avvio.
    pub restart_len: usize,
    /// Margine (logaritmo) oltre il quale il controllo periodico rifa' l'avvio; 0 = spento.
    pub recheck_margin: f64,
    /// BigScan: soglia del contrasto locale integrato e passi di silenzio prima di chiudere un canale inattivo.
    pub band_peak: f32,
    pub band_idle_steps: u32,
    /// BigScan: righe mascherate attorno ai canali forti e potenza (riferita al rumore) oltre la quale un canale e' forte.
    pub band_mask_strong: usize,
    pub band_strong: f32,
}

impl Default for ChainParams {
    fn default() -> Self {
        ChainParams {
            window_fc: 25.0,
            gate_on: 0.5,
            gate_off: 0.4,
            len_long: 71,
            len_max: 17,
            len_short: 9,
            len_min: 111,
            floor_max: 3.0,
            env_base: 0.8,
            env_slope: 0.35,
            peak_k: 0.5,
            tanh_floor: 0.3,
            tanh_gain: 1.0,
            p_closed: 0.003,
            emit_mode: 1,
            rice_weight: 0.8,
            rice_min: 6.0,
            side_k: 3.0,
            a2_short: true,
            iq_fc: 60.0,
            coh_piece_units: 2.0,
            coh_snr_cap: 20.0,
            coh_max_pieces: 16,
            coh_min: 2.0,
            coh_closed: 12.0,
            emit_weight: 1.0,
            class_w: [0.58, 0.42, 0.65, 0.23, 0.12],
            tail3: 0.0001,
            tail4: 0.1,
            r_scale: 1.0,
            q_frac: 0.003,
            r_floor: 0.0,
            r_cap: 0.5,
            learn_elements: 30,
            seg_lookahead: 0,
            lex_weight: 1.0,
            flush_steps: 40,
            restart_len: 7,
            recheck_margin: 10.0,
            band_peak: 7.5,
            band_idle_steps: 200,
            band_mask_strong: 5,
            band_strong: 100.0,
        }
    }
}

impl ChainParams {
    /// Cambia un parametro per nome (per le prove sul banco); `false` se il nome non c'e'.
    pub fn set(&mut self, name: &str, v: f64) -> bool {
        let f = v as f32;
        let u = v.max(0.0) as usize;
        match name {
            "window_fc" => self.window_fc = v,
            "gate_on" => self.gate_on = f,
            "gate_off" => self.gate_off = f,
            "len_long" => self.len_long = u.max(1),
            "len_max" => self.len_max = u.max(1),
            "len_short" => self.len_short = u.max(1),
            "len_min" => self.len_min = u.max(1),
            "floor_max" => self.floor_max = f,
            "env_base" => self.env_base = f,
            "env_slope" => self.env_slope = f,
            "peak_k" => self.peak_k = f,
            "tanh_floor" => self.tanh_floor = f,
            "tanh_gain" => self.tanh_gain = f,
            "p_closed" => self.p_closed = f,
            "emit_weight" => self.emit_weight = v,
            "emit_mode" => self.emit_mode = u as u8,
            "iq_fc" => self.iq_fc = v,
            "coh_piece_units" => self.coh_piece_units = v,
            "coh_snr_cap" => self.coh_snr_cap = v,
            "coh_max_pieces" => self.coh_max_pieces = u.max(1),
            "coh_min" => self.coh_min = v.max(0.01),
            "coh_closed" => self.coh_closed = v.max(0.0),
            "rice_weight" => self.rice_weight = v,
            "rice_min" => self.rice_min = v,
            "side_k" => self.side_k = f.max(0.1),
            "a2_short" => self.a2_short = v != 0.0,
            "w0" => self.class_w[0] = v,
            "w1" => self.class_w[1] = v,
            "w2" => self.class_w[2] = v,
            "w3" => self.class_w[3] = v,
            "w4" => self.class_w[4] = v,
            "tail3" => self.tail3 = v,
            "tail4" => self.tail4 = v,
            "r_scale" => self.r_scale = v,
            "q_frac" => self.q_frac = v,
            "r_floor" => self.r_floor = v,
            "r_cap" => self.r_cap = v,
            "learn_elements" => self.learn_elements = u,
            "seg_lookahead" => self.seg_lookahead = u,
            "lex_weight" => self.lex_weight = v,
            "flush_steps" => self.flush_steps = u.max(1),
            "restart_len" => self.restart_len = u.max(1),
            "recheck_margin" => self.recheck_margin = v,
            "band_peak" => self.band_peak = f,
            "band_idle_steps" => self.band_idle_steps = u as u32,
            "band_mask_strong" => self.band_mask_strong = u,
            "band_strong" => self.band_strong = f,
            _ => return false,
        }
        true
    }

    /// Applica una lista `nome=valore,nome=valore`; restituisce i nomi sconosciuti.
    pub fn apply_list(&mut self, list: &str) -> Vec<String> {
        let mut bad = Vec::new();
        for kv in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let ok = kv.split_once('=').and_then(|(k, v)| v.trim().parse::<f64>().ok().map(|v| (k.trim(), v))).is_some_and(|(k, v)| self.set(k, v));
            if !ok {
                bad.push(kv.to_string());
            }
        }
        bad
    }
}

// ---------------------------------------------------------------- stadio 1: spettro

/// La finestra: passa-basso a sinc con taglio `fc` per una finestra a quattro coseni, lunga `NFFT`.
fn chain_window(fc: f64) -> Vec<f32> {
    const A: [f64; 4] = [0.35875, 0.48829, 0.14128, 0.01168];
    let n1 = (NFFT - 1) as f64;
    let mid = n1 / 2.0;
    (0..NFFT)
        .map(|n| {
            let x = 2.0 * PI * fc * (n as f64 - mid) / RATE;
            let sinc = if x.abs() < 1e-12 { 1.0 } else { x.sin() / x };
            let t = 2.0 * PI * n as f64 / n1;
            let w = A[0] - A[1] * t.cos() + A[2] * (2.0 * t).cos() - A[3] * (3.0 * t).cos();
            (sinc * w) as f32
        })
        .collect()
}

/// Spettro di potenza a cadenza fissa, con il rumore della banda (mediana delle righe) come riferimento.
struct Spectrum {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    ring: Vec<Complex32>,
    pos: usize,
    since: usize,
    filled: usize,
    buf: Vec<Complex32>,
    scratch: Vec<Complex32>,
    power: Vec<f32>,
    /// Righe usate per il rumore.
    noise_lo: usize,
    noise_hi: usize,
    sel: Vec<f32>,
    /// Potenza di rumore per riga (media stimata dalla mediana) e quanti spettri visti.
    noise: f32,
    count: u64,
    /// Spostamento di frequenza in ingresso (passo di fase per campione) e fase.
    shift: f64,
    phase: f64,
    /// Ibrido coerente: seconda finestra (passa-basso piu' largo), righe complesse e rumore per riga di quella finestra.
    iq_window: Vec<f32>,
    iq_buf: Vec<Complex32>,
    iq: Vec<Complex32>,
    iq_noise: f32,
}

impl Spectrum {
    fn new(fc: f64, fc_iq: f64, shift_hz: f64, noise_lo_hz: f64, noise_hi_hz: f64) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(NFFT);
        let scratch = vec![Complex32::new(0.0, 0.0); fft.get_inplace_scratch_len()];
        let noise_lo = ((noise_lo_hz / BIN_HZ) as usize).max(1);
        let noise_hi = ((noise_hi_hz / BIN_HZ) as usize).clamp(noise_lo + 8, NFFT / 2 - 1);
        Spectrum {
            fft,
            window: chain_window(fc),
            ring: vec![Complex32::new(0.0, 0.0); NFFT],
            pos: 0,
            since: 0,
            filled: 0,
            buf: vec![Complex32::new(0.0, 0.0); NFFT],
            scratch,
            power: vec![0.0; NFFT / 2],
            noise_lo,
            noise_hi,
            sel: Vec::with_capacity(noise_hi - noise_lo),
            noise: 0.0,
            count: 0,
            shift: 2.0 * PI * shift_hz / RATE,
            phase: 0.0,
            iq_window: if fc_iq > 0.0 { chain_window(fc_iq) } else { Vec::new() },
            iq_buf: vec![Complex32::new(0.0, 0.0); NFFT],
            iq: if fc_iq > 0.0 { vec![Complex32::new(0.0, 0.0); NFFT / 2] } else { Vec::new() },
            iq_noise: 0.0,
        }
    }

    /// La riga `k` della finestra coerente riportata in fase fra uno spettro e l'altro (una riga ruota di 2 pi k / 8 a ogni passo) e
    /// riferita al rumore: un campione di banda base a 93,75 Hz con varianza del rumore circa 1.
    fn baseband(&self, k: usize) -> Option<Complex32> {
        baseband_of(&self.iq, k, self.count, self.iq_noise)
    }

    /// Un campione; `true` quando c'e' uno spettro nuovo in `power` (e `noise` aggiornato).
    fn push(&mut self, x: f32) -> bool {
        let z = if self.shift == 0.0 {
            Complex32::new(x, 0.0)
        } else {
            self.phase += self.shift;
            if self.phase > PI {
                self.phase -= 2.0 * PI;
            } else if self.phase < -PI {
                self.phase += 2.0 * PI;
            }
            Complex32::new(x * self.phase.cos() as f32, x * self.phase.sin() as f32)
        };
        self.ring[self.pos] = z;
        self.pos = (self.pos + 1) % NFFT;
        self.filled = (self.filled + 1).min(NFFT);
        self.since += 1;
        if self.since < HOP {
            return false;
        }
        self.since = 0;
        for i in 0..NFFT {
            self.buf[i] = self.ring[(self.pos + i) % NFFT] * self.window[i];
        }
        self.fft.process_with_scratch(&mut self.buf, &mut self.scratch);
        for (p, c) in self.power.iter_mut().zip(&self.buf) {
            *p = c.norm_sqr();
        }
        self.sel.clear();
        self.sel.extend_from_slice(&self.power[self.noise_lo..self.noise_hi]);
        let k = self.sel.len() / 2;
        let med = if self.sel.is_empty() { 0.0 } else { *self.sel.select_nth_unstable_by(k, f32::total_cmp).1 };
        // la mediana di una potenza esponenziale e' ln 2 volte la media
        let est = med / std::f32::consts::LN_2;
        self.count += 1;
        let a = if self.count < 32 { 1.0 / self.count as f32 } else { 0.03 };
        self.noise += a * (est - self.noise);
        if !self.iq_window.is_empty() {
            for i in 0..NFFT {
                self.iq_buf[i] = self.ring[(self.pos + i) % NFFT] * self.iq_window[i];
            }
            self.fft.process_with_scratch(&mut self.iq_buf, &mut self.scratch);
            self.iq.copy_from_slice(&self.iq_buf[..NFFT / 2]);
            self.sel.clear();
            self.sel.extend(self.iq[self.noise_lo..self.noise_hi].iter().map(|c| c.norm_sqr()));
            let k = self.sel.len() / 2;
            let med = if self.sel.is_empty() { 0.0 } else { *self.sel.select_nth_unstable_by(k, f32::total_cmp).1 };
            let est = med / std::f32::consts::LN_2;
            self.iq_noise += a * (est - self.iq_noise);
        }
        true
    }
}

/// La riga `k` delle righe complesse `iq` dello spettro numero `count`, in fase e riferita al rumore `noise`.
fn baseband_of(iq: &[Complex32], k: usize, count: u64, noise: f32) -> Option<Complex32> {
    let z = *iq.get(k)?;
    let turn = ((k as u64 * count) % 8) as f64 * 2.0 * PI / 8.0;
    let r = Complex32::new(turn.cos() as f32, -turn.sin() as f32);
    Some(z * r / noise.max(1e-30).sqrt())
}

/// Esito del confronto di forma attorno al tono (grandezze d'ampiezza riferite al rumore).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shape {
    /// Ampiezza dalla riga centrale.
    pub a: f32,
    /// Fondo dalle spalle simmetriche.
    pub n: f32,
    /// Residuo: ampiezza - fondo.
    pub d: f32,
    /// Potenza media delle spalle lontane (righe 6-8 da ogni lato), riferita al rumore.
    pub side: f32,
}

/// Confronto di forma su ±10 righe attorno a `center`: ampiezza dalla riga centrale, fondo dai prodotti delle coppie di spalle 4-8.
pub fn match_shape(power: &[f32], center: usize, noise: f32) -> Shape {
    let r = 1.0 / (noise.max(1e-30) + 1e-12);
    let q = |i: isize| -> f32 {
        let k = center as isize + i;
        if k < 0 || k as usize >= power.len() { 1.0 } else { power[k as usize] * r }
    };
    let mut s = 0.0f32;
    let mut side = 0.0f32;
    for i in 4..=8isize {
        s += (q(-i) * q(i) + 1e-12).log2();
    }
    // per l'ibrido le spalle lontane (6-8 righe), fuori dalla dispersione del tono anche molto forte
    for i in 6..=8isize {
        side += q(-i) + q(i);
    }
    let n = (s / 16.0).exp();
    let a = q(0).max(0.0).sqrt();
    Shape { a, n, d: a - n, side: side / 6.0 }
}

// ---------------------------------------------------------------- stadio 2: inviluppo

/// Media rettangolare scorrevole.
#[derive(Clone)]
struct BoxMean {
    buf: VecDeque<f32>,
    len: usize,
    sum: f64,
}

impl BoxMean {
    fn new(len: usize) -> Self {
        BoxMean { buf: VecDeque::with_capacity(len + 1), len: len.max(1), sum: 0.0 }
    }

    fn push(&mut self, x: f32) -> f32 {
        self.buf.push_back(x);
        self.sum += f64::from(x);
        if self.buf.len() > self.len {
            self.sum -= f64::from(self.buf.pop_front().unwrap_or(0.0));
        }
        (self.sum / self.len as f64) as f32
    }
}

/// Massimo (o minimo) scorrevole con la coda monotona.
#[derive(Clone)]
struct SlidingExt {
    q: VecDeque<(u64, f32)>,
    len: u64,
    t: u64,
    max: bool,
}

impl SlidingExt {
    fn new(len: usize, max: bool) -> Self {
        SlidingExt { q: VecDeque::new(), len: len.max(1) as u64, t: 0, max }
    }

    fn push(&mut self, x: f32) -> f32 {
        while self.q.back().is_some_and(|&(_, v)| if self.max { v <= x } else { v >= x }) {
            self.q.pop_back();
        }
        self.q.push_back((self.t, x));
        while self.q.front().is_some_and(|&(t, _)| t + self.len <= self.t) {
            self.q.pop_front();
        }
        self.t += 1;
        self.q.front().map_or(x, |f| f.1)
    }
}

/// Inseguimento dell'inviluppo e probabilita' di tasto abbassato (stadi 2 e 3).
#[derive(Clone)]
struct Tracker {
    a_hist: [f32; 3],
    d_hist: [f32; 3],
    m_hist: [f32; 3],
    side_hist: [f32; 3],
    long: BoxMean,
    max: SlidingExt,
    short: BoxMean,
    min: SlidingExt,
    gate: bool,
    a_max: SlidingExt,
    a_short: BoxMean,
    a_env: f32,
    long_v: f32,
    floor: f32,
    /// Media dei quadrati dell'ampiezza nei passi sicuri e quanti.
    msq: f64,
    msq_n: u32,
}

/// Uscita di un passo dell'inviluppo.
#[derive(Clone, Copy, Debug)]
pub struct KeyProb {
    /// Probabilita' di tasto abbassato.
    pub p: f32,
    /// Cancello di presenza aperto.
    pub gate: bool,
    /// Qualita' bassa (media lunga sotto 2).
    pub weak: bool,
}

impl Tracker {
    fn new(pr: &ChainParams) -> Self {
        Tracker {
            a_hist: [0.0; 3],
            d_hist: [0.0; 3],
            m_hist: [0.0; 3],
            side_hist: [1.0; 3],
            long: BoxMean::new(pr.len_long),
            max: SlidingExt::new(pr.len_max, true),
            short: BoxMean::new(pr.len_short),
            min: SlidingExt::new(pr.len_min, false),
            gate: false,
            a_max: SlidingExt::new(pr.len_max, true),
            a_short: BoxMean::new(pr.len_short),
            a_env: 0.0,
            long_v: 0.0,
            floor: pr.floor_max,
            msq: 1.0,
            msq_n: 0,
        }
    }

    fn step(&mut self, s: Shape, pr: &ChainParams) -> KeyProb {
        self.a_hist = [self.a_hist[1], self.a_hist[2], s.a];
        self.side_hist = [self.side_hist[1], self.side_hist[2], s.side];
        self.d_hist = [self.d_hist[1], self.d_hist[2], s.d.max(0.0)];
        let [x, y, z] = self.d_hist;
        let m = median3(x, y, z);
        self.long_v = self.long.push(m);
        self.gate = if self.gate { self.long_v > pr.gate_off } else { self.long_v > pr.gate_on };
        let env = self.short.push(self.max.push(m));
        // inviluppo superiore dell'ampiezza (per l'ibrido): segue l'evanescenza in qualche decimo di secondo
        self.a_env = self.a_short.push(self.a_max.push(s.a));
        self.m_hist = [self.m_hist[1], self.m_hist[2], m];
        // il fondo scende sotto il massimo solo sui picchi del tono (massimo locale ben sopra la media lunga)
        let [m0, m1, m2] = self.m_hist;
        let peak = m1 >= m0 && m1 >= m2 && m1 > pr.peak_k * (pr.floor_max + self.long_v);
        let cand = if self.gate && peak { pr.floor_max.min(pr.env_base + pr.env_slope * (env - pr.env_base).max(0.0)) } else { pr.floor_max };
        self.floor = self.min.push(cand);
        // le due grandezze allineate al centro della mediana
        let (a, d) = (self.a_hist[1], self.d_hist[1]);
        let p = if self.gate {
            let g = pr.tanh_gain;
            0.25 * ((g * (a.max(pr.tanh_floor) - self.floor)).tanh() + (g * (d.max(pr.tanh_floor) - self.floor)).tanh() + 2.0)
        } else {
            pr.p_closed
        };
        if p > 0.8 {
            self.msq_n = (self.msq_n + 1).min(4096);
            self.msq += (f64::from(a * a) - self.msq) / f64::from(self.msq_n);
        }
        let p = if pr.emit_mode == 1 && self.gate {
            // verosimiglianza di Rice contro Rayleigh sulla potenza della riga riferita al rumore locale delle spalle
            // le spalle contano solo quando salgono molto (scariche, disturbi larghi): da sole sono una stima rumorosa
            // e solo se arrivano vicino al centro: la dispersione di un tono fortissimo non e' un disturbo largo
            let sd = self.side_hist[1];
            let side = if sd > pr.side_k && sd * 30.0 > a * a { sd / pr.side_k } else { 1.0 };
            let x = f64::from(a * a / side);
            let a2 = if pr.a2_short { f64::from(self.a_env * self.a_env).min(self.msq) - 1.0 } else { self.msq - 1.0 }.max(pr.rice_min);
            let z = 2.0 * (a2 * x).sqrt();
            let llr = -a2 + ln_i0(z);
            let l = (pr.rice_weight * llr).clamp(-30.0, 30.0);
            (1.0 / (1.0 + (-l).exp())) as f32
        } else {
            p
        };
        KeyProb { p, gate: self.gate, weak: self.long_v < 2.0 }
    }
}

/// Logaritmo della funzione di Bessel modificata I0.
fn ln_i0(z: f64) -> f64 {
    if z < 3.75 {
        let t = (z / 3.75).powi(2);
        (1.0 + t * (3.515_622_9 + t * (3.089_942_4 + t * (1.206_749_2 + t * (0.265_973_2 + t * (0.036_076_8 + t * 0.004_581_3)))))).ln()
    } else {
        let t = 3.75 / z;
        let poly = 0.398_942_28 + t * (0.013_285_92 + t * (0.002_253_19 + t * (-0.001_575_65 + t * (0.009_162_81 + t * (-0.020_577_06 + t * (0.026_355_37 + t * (-0.016_476_33 + t * 0.003_923_77)))))));
        z - 0.5 * z.ln() + poly.ln()
    }
}

fn median3(x: f32, y: f32, z: f32) -> f32 {
    x.min(y).max(z.min(x.max(y)))
}

// ---------------------------------------------------------------- stadio 5: modello delle durate

/// Medie di partenza (logaritmo della durata in passi) delle cinque classi: punto, linea, pausa breve, fra i caratteri, fra le parole.
const MU0: [f64; 5] = [1.62, 2.70, 1.50, 2.72, 3.56];
/// Covarianza a priori delle cinque medie.
const P0: [[f64; 5]; 5] = [
    [0.0567, 0.0492, 0.0454, 0.0474, 0.0367],
    [0.0492, 0.0509, 0.0508, 0.0502, 0.0385],
    [0.0454, 0.0508, 0.0710, 0.0602, 0.0464],
    [0.0474, 0.0502, 0.0602, 0.2000, 0.0612],
    [0.0367, 0.0385, 0.0464, 0.0612, 0.2000],
];
/// Varianze di osservazione di partenza.
const R0: [f64; 5] = [0.0567, 0.0509, 0.0710, 0.2, 0.2];
/// Logaritmo della durata massima creduta (passi).
const LN_MAX: f64 = 5.703_782_474_656_201; // ln 300

/// Le cinque classi di durata con l'adattamento correlato delle medie.
#[derive(Clone, Debug)]
pub struct DurationModel {
    pub mu: [f64; 5],
    p: [[f64; 5]; 5],
    r: [f64; 5],
    w: [f64; 5],
    tail3: f64,
    tail4: f64,
    q_frac: f64,
    r_floor: f64,
    r_cap: f64,
    learning_left: usize,
}

impl DurationModel {
    /// Il modello di partenza per la velocita' `wpm`.
    pub fn new(wpm: f64, pr: &ChainParams) -> Self {
        let unit = (112.5 / wpm.clamp(5.0, 60.0)).ln();
        let shift = unit - MU0[0];
        let mut mu = MU0;
        for m in &mut mu {
            *m += shift;
        }
        DurationModel {
            mu,
            p: P0,
            r: R0.map(|r| r * pr.r_scale),
            w: pr.class_w,
            tail3: pr.tail3,
            tail4: pr.tail4,
            q_frac: pr.q_frac,
            r_floor: pr.r_floor,
            r_cap: pr.r_cap,
            learning_left: pr.learn_elements,
        }
    }

    /// Logaritmo della lognormale della classe `j` (densita' in passi), senza peso.
    fn log_lognormal(&self, j: usize, d: f64) -> f64 {
        let ln = d.max(1e-3).ln();
        let x = ln - self.mu[j];
        let v = self.r[j].max(1e-6);
        -x * x / (2.0 * v) - 0.5 * (2.0 * PI * v).ln() - ln
    }

    /// Logaritmo della densita' pesata della classe `j` alla durata `d` (passi), code uniformi comprese.
    pub fn log_density(&self, j: usize, d: f64) -> f64 {
        let ln = d.max(1e-3).ln();
        let lse = |a: f64, b: f64| {
            let m = a.max(b);
            if m == f64::NEG_INFINITY { m } else { m + ((a - m).exp() + (b - m).exp()).ln() }
        };
        let wl = |w: f64| if w > 0.0 { w.ln() } else { f64::NEG_INFINITY };
        match j {
            3 => {
                let lo = self.mu[2] + 3.0 * self.p[2][2].sqrt();
                let hi = self.mu[3] + 3.0 * self.p[4][4].sqrt();
                let u = if ln > lo && ln < hi && hi > lo { wl(self.tail3) - (hi - lo).ln() - ln } else { f64::NEG_INFINITY };
                lse(wl(self.w[3] - self.tail3) + self.log_lognormal(3, d), u)
            }
            4 => {
                let lo = self.mu[3] + 3.0 * self.p[3][3].sqrt();
                let u = if ln > lo { wl(self.tail4) - (300.0 - lo).ln() - ln } else { f64::NEG_INFINITY };
                lse(wl(self.w[4] - self.tail4) + self.log_lognormal(4, d), u)
            }
            _ => wl(self.w[j]) + self.log_lognormal(j, d),
        }
    }

    /// Densita' pesata della classe `j` alla durata `d` (passi).
    pub fn density(&self, j: usize, d: f64) -> f64 {
        self.log_density(j, d).exp()
    }

    /// Classe di un tono (0 punto, 1 linea).
    pub fn classify_on(&self, d: f64) -> usize {
        let ln = d.max(1e-3).ln();
        if ln <= self.mu[0] {
            return 0;
        }
        if ln >= self.mu[1] {
            return 1;
        }
        if self.log_density(0, d) > self.log_density(1, d) { 0 } else { 1 }
    }

    /// Classe di una pausa (2, 3 o 4) e probabilita' che sia fra le parole rispetto a fra i caratteri.
    pub fn classify_off(&self, d: f64) -> (usize, f64) {
        let (p2, p3, p4) = (self.log_density(2, d), self.log_density(3, d), self.log_density(4, d));
        let pw = if p3 == f64::NEG_INFINITY && p4 == f64::NEG_INFINITY { 0.0 } else { 1.0 / (1.0 + (p3 - p4).exp()) };
        let ln = d.max(1e-3).ln();
        // le classi restano in ordine di durata anche lontano dalle medie
        let c = if ln <= self.mu[2] || (ln < self.mu[3] && p2 >= p3 && p2 >= p4) {
            2
        } else if ln <= self.mu[3] || (ln < self.mu[4] && p3 >= p4) {
            3
        } else {
            4
        };
        (c, pw)
    }

    /// Aggiornamento correlato con l'osservazione `d` (passi) della classe `j`.
    pub fn update(&mut self, j: usize, d: f64) {
        let z = d.max(1.0).ln().min(LN_MAX);
        let e = self.mu[j] - z;
        let (keep, add) = if self.learning_left > 0 { (0.5, 0.5) } else { (0.8, 0.2) };
        self.learning_left = self.learning_left.saturating_sub(1);
        let c = if j == 4 { 1.0f64.max(self.r_cap) } else { self.r_cap };
        let m = self.mu[j].abs().max(0.3);
        self.r[j] = (keep * self.r[j] + add * e * e).clamp((0.15 / m).powi(2).max(self.r_floor), (c / m).powi(2).max(self.r_floor));
        let mut pt = self.p;
        for (row, prow) in pt.iter_mut().zip(&P0) {
            for (x, p0) in row.iter_mut().zip(prow) {
                *x += self.q_frac * p0;
            }
        }
        let s = self.r[j] + pt[j][j];
        if s <= 0.0 {
            return;
        }
        let col: [f64; 5] = std::array::from_fn(|i| pt[i][j]);
        for i in 0..5 {
            for k in 0..5 {
                self.p[i][k] = pt[i][k] - col[i] * col[k] / s;
            }
        }
        let innov = z - self.mu[j];
        for (m, c) in self.mu.iter_mut().zip(col) {
            *m += c / s * innov;
        }
        // le medie restano in ordine: la linea dopo il punto, le pause lunghe dopo la breve
        self.mu[1] = self.mu[1].max(self.mu[0] + 0.6);
        self.mu[3] = self.mu[3].max(self.mu[2] + 0.6);
        self.mu[4] = self.mu[4].max(self.mu[3] + 0.3);
    }

    /// Quanto il modello spiega una serie di elementi (somma dei logaritmi della classe migliore di ognuno).
    fn fit(&self, els: &VecDeque<(bool, usize)>) -> f64 {
        els.iter()
            .map(|&(on, len)| {
                let d = len as f64;
                let cls: &[usize] = if on { &[0, 1] } else { &[2, 3, 4] };
                cls.iter().map(|&j| self.log_density(j, d)).fold(f64::NEG_INFINITY, f64::max).max(-50.0)
            })
            .sum()
    }

    /// Riparte dalle medie `mu` con la covarianza e le varianze di partenza.
    fn restart(&mut self, mu: [f64; 5], pr: &ChainParams) {
        self.mu = mu;
        self.p = P0;
        self.r = R0.map(|r| r * pr.r_scale);
        self.learning_left = pr.learn_elements;
    }

    /// Velocita' (WPM) da punto, linea e pausa breve.
    pub fn wpm(&self) -> f64 {
        112.5 * (3.0 * 0.42 + 0.58 + 0.65) / (0.58 * self.mu[0].exp() + 0.42 * self.mu[1].exp() + 0.65 * self.mu[2].exp())
    }

    /// Tabelle della decodifica di sequenza: per ogni età, logaritmo di restare e di finire (l'ultima età e' la coda).
    fn hazards(&self, on: bool, ages: usize) -> (Vec<f64>, Vec<f64>) {
        let classes: &[usize] = if on { &[0, 1] } else { &[2, 3, 4] };
        let dmax = 600usize;
        let pmf: Vec<f64> = (1..=dmax).map(|d| classes.iter().map(|&j| self.density(j, d as f64)).sum::<f64>()).collect();
        let tot: f64 = pmf.iter().sum::<f64>().max(1e-300);
        let mut stay = Vec::with_capacity(ages);
        let mut end = Vec::with_capacity(ages);
        let mut surv = 1.0f64;
        for &f in pmf.iter().take(ages - 1) {
            let h = (f / tot / surv.max(1e-300)).clamp(1e-9, 1.0 - 1e-9);
            stay.push((1.0 - h).ln());
            end.push(h.ln());
            surv *= 1.0 - h;
        }
        // coda: vita media residua oltre l'ultima età
        let rest: f64 = pmf.iter().skip(ages - 1).sum::<f64>() / tot;
        let mut life = 0.0;
        let mut s = rest;
        for &f in pmf.iter().skip(ages - 1) {
            life += s / rest.max(1e-300);
            s -= f / tot;
        }
        let h = (1.0 / life.max(1.0)).clamp(1e-9, 1.0 - 1e-9);
        stay.push((1.0 - h).ln());
        end.push(h.ln());
        (stay, end)
    }
}

/// Avvio: le cinque medie (logaritmo della durata in passi) che spiegano meglio i primi elementi, pesando tutte le ipotesi di
/// divisione di toni e pause in classi. Toni e pause hanno ciascuno la propria unita' (il rivelatore allunga gli uni e accorcia le
/// altre), legate debolmente fra loro; le distanze fra le classi seguono quelle di partenza con una certa liberta'.
/// `on` e `off` sono durate (passi) di toni e pause.
pub fn startup_means(on: &[f64], off: &[f64], prior_unit: f64) -> [f64; 5] {
    let rel = MU0.map(|m| m - MU0[0]);
    let mut xs: Vec<f64> = on.iter().map(|d| d.max(1.0).ln()).collect();
    let mut ys: Vec<f64> = off.iter().map(|d| d.max(1.0).ln().min(LN_MAX)).collect();
    xs.sort_by(f64::total_cmp);
    ys.sort_by(f64::total_cmp);
    let mean = |v: &[f64]| if v.is_empty() { None } else { Some(v.iter().sum::<f64>() / v.len() as f64) };
    let mut best = f64::NEG_INFINITY;
    let mut scored: Vec<(f64, [f64; 5])> = Vec::new();
    for k in 0..=xs.len() {
        for i in 0..=ys.len() {
            for j in i..=ys.len() {
                let groups: [&[f64]; 5] = [&xs[..k], &xs[k..], &ys[..i], &ys[i..j], &ys[j..]];
                let unit_of = |cls: &[usize]| {
                    let v: Vec<f64> = cls.iter().flat_map(|&c| groups[c].iter().map(move |x| x - rel[c])).collect();
                    mean(&v)
                };
                let u_on = unit_of(&[0, 1]).unwrap_or(prior_unit);
                let u_off = unit_of(&[2, 3]).or_else(|| unit_of(&[4])).unwrap_or(u_on);
                let mut mu = [0.0; 5];
                let mut score = -(u_on - prior_unit).powi(2) / (2.0 * 0.5) - (u_on - u_off).powi(2) / (2.0 * 0.3);
                // quante durate per classe: le pause brevi sono le piu' frequenti, punti e linee si equivalgono
                const SHARE: [f64; 5] = [0.5, 0.5, 0.55, 0.33, 0.12];
                for (g, sh) in groups.iter().zip(SHARE) {
                    score += g.len() as f64 * sh.ln();
                }
                for c in 0..5 {
                    let u = if c < 2 { u_on } else { u_off };
                    let m = mean(groups[c]).unwrap_or(u + rel[c]);
                    mu[c] = m;
                    if c < 4 {
                        score -= groups[c].iter().map(|x| (x - m).powi(2)).sum::<f64>() / (2.0 * 0.03);
                    }
                    if !groups[c].is_empty() {
                        score -= (m - u - rel[c]).powi(2) / (2.0 * 0.08);
                    }
                }
                // le classi restano in ordine
                if mu[1] <= mu[0] || mu[3] <= mu[2] || mu[4] <= mu[3] {
                    score -= 50.0;
                }
                best = best.max(score);
                scored.push((score, mu));
            }
        }
    }
    let mut wsum = 0.0;
    let mut acc = [0.0; 5];
    for (sc, mu) in scored {
        let w = (sc - best).exp();
        wsum += w;
        for (a, m) in acc.iter_mut().zip(mu) {
            *a += w * m;
        }
    }
    if wsum > 0.0 { acc.map(|a| a / wsum) } else { MU0.map(|m| m - MU0[0] + prior_unit) }
}

// ---------------------------------------------------------------- stadio 4: sequenza a durate esplicite

#[derive(Clone, Copy, Default)]
struct Back {
    on0_from: u8,
    off0_from: u8,
    on_tail_self: bool,
    off_tail_self: bool,
}

/// Decodifica di sequenza a durate esplicite: età del tono e del silenzio, code, percorso migliore con risalita.
#[derive(Clone)]
struct Sequence {
    on: [f64; ON_AGES],
    off: [f64; OFF_AGES],
    back: VecDeque<Back>,
    on_stay: Vec<f64>,
    on_end: Vec<f64>,
    off_stay: Vec<f64>,
    off_end: Vec<f64>,
    /// Ibrido coerente: punteggio d'ingresso di ogni eta' del tono (prima del giudizio del tono), stato della coda (ingresso, primo
    /// passo, passi oltre l'ultima eta'), somme progressive della banda base e passi visti.
    base: [f64; ON_AGES],
    tail: (f64, u64, u32),
    cum: Vec<Complex64>,
    t: u64,
}

/// Complesso in doppia precisione per le somme progressive.
#[derive(Clone, Copy, Default, Debug)]
pub struct Complex64 {
    pub re: f64,
    pub im: f64,
}

/// Passi tenuti nelle somme progressive della banda base (un tono non dura di piu').
const CUM_LEN: usize = 1024;

/// Come si giudica un tono nell'ibrido coerente.
#[derive(Clone, Copy, Debug)]
pub struct CohJudge<'a> {
    /// Ampiezza del tono al quadrato, per passo, riferita al rumore.
    pub amp2: f64,
    /// Rumore (varianza per passo) da credere: almeno 1, di piu' quando il tono e' oltre lo S/N massimo creduto.
    pub nu: f64,
    /// Passi di un pezzo sommato in fase e pezzi al massimo.
    pub piece: usize,
    pub max_pieces: usize,
    /// Penalita' (logaritmo) per aprire un tono a cancello chiuso.
    pub closed_penalty: f64,
    /// Varianza del rumore di una somma di `n` passi (correlati dalla finestra), indice `n`.
    pub var_sum: &'a [f64],
}

/// Fattori dell'ampiezza (sulla potenza) e pesi della miscela, e peso dell'ampiezza che spiega meglio il tono stesso.
const COH_AMP: [f64; 4] = [0.3, 0.6, 1.0, 1.6];
const COH_W: [f64; 4] = [0.1, 0.1, 0.65, 0.15];
const COH_ML: f64 = 0.03;

/// Varianza del rumore di una somma di `n` passi consecutivi della banda base, dalla finestra (passi sovrapposti, quindi correlati).
pub fn coherent_var_sum(window: &[f32], max: usize) -> Vec<f64> {
    let r = |m: usize| -> f64 { window.iter().zip(window.iter().skip(m)).map(|(a, b)| f64::from(*a) * f64::from(*b)).sum() };
    let r0 = r(0).max(1e-30);
    let rho: Vec<f64> = (0..).map(|k| k * HOP).take_while(|&m| m < window.len()).map(|m| r(m) / r0).collect();
    (0..=max)
        .map(|n| {
            let nf = n as f64;
            nf + 2.0 * rho.iter().enumerate().skip(1).filter(|(k, _)| *k < n).map(|(k, p)| (nf - k as f64) * p).sum::<f64>()
        })
        .collect()
}

/// Logaritmo del rapporto di verosimiglianza di un tono (Rice contro Rayleigh) con le somme in fase dei suoi pezzi: `pieces` sono
/// (passi, modulo della somma). Miscela su quattro ampiezze attorno alla stima e su quella che spiega meglio il tono.
pub fn coherent_llr(pieces: &[(usize, f64)], j: &CohJudge) -> f64 {
    let var_of = |n: usize| j.var_sum.get(n).copied().unwrap_or(n as f64) * j.nu;
    let total_n: usize = pieces.iter().map(|p| p.0).sum();
    if total_n == 0 {
        return 0.0;
    }
    let coherent: f64 = pieces.iter().map(|p| p.1).sum();
    let ml = (coherent / total_n as f64).powi(2);
    let mut acc = [0.0f64; COH_AMP.len() + 1];
    let amps: [f64; COH_AMP.len() + 1] = std::array::from_fn(|i| if i < COH_AMP.len() { (j.amp2 * COH_AMP[i]).sqrt() } else { ml.sqrt() });
    for &(n, mag) in pieces {
        if n == 0 {
            continue;
        }
        let var = var_of(n);
        for (a, x) in amps.iter().zip(acc.iter_mut()) {
            let mu = n as f64 * a;
            *x += ln_i0(2.0 * mu * mag / var) - mu * mu / var;
        }
    }
    let best = acc.iter().copied().fold(f64::MIN, f64::max);
    let weights = COH_W.iter().map(|w| w * (1.0 - COH_ML)).chain(std::iter::once(COH_ML));
    best + acc.iter().zip(weights).map(|(l, w)| w * (l - best).exp()).sum::<f64>().ln()
}

impl Sequence {
    fn new(model: &DurationModel) -> Self {
        let mut s = Sequence {
            on: [NEG; ON_AGES],
            off: [NEG; OFF_AGES],
            back: VecDeque::with_capacity(HISTORY + DECIDE + 1),
            on_stay: Vec::new(),
            on_end: Vec::new(),
            off_stay: Vec::new(),
            off_end: Vec::new(),
            base: [NEG; ON_AGES],
            tail: (NEG, 0, 0),
            cum: vec![Complex64::default(); CUM_LEN],
            t: 0,
        };
        s.off[OFF_AGES - 1] = 0.0;
        s.retable(model);
        s
    }

    fn retable(&mut self, m: &DurationModel) {
        (self.on_stay, self.on_end) = m.hazards(true, ON_AGES);
        (self.off_stay, self.off_end) = m.hazards(false, OFF_AGES);
    }

    /// Riparte in silenzio lungo.
    fn reset(&mut self) {
        self.base = [NEG; ON_AGES];
        self.tail = (NEG, 0, 0);
        self.on = [NEG; ON_AGES];
        self.off = [NEG; OFF_AGES];
        self.off[OFF_AGES - 1] = 0.0;
        self.back.clear();
    }

    fn step(&mut self, p: f32, weight: f64) {
        let p = f64::from(p).clamp(1e-4, 1.0 - 1e-4);
        let (e_on, e_off) = (weight * p.ln(), weight * (1.0 - p).ln());
        let mut b = Back::default();
        let (mut best_off_end, mut arg_off) = (f64::NEG_INFINITY, 0usize);
        for (d, (&s, &h)) in self.off.iter().zip(&self.off_end).enumerate() {
            if s + h > best_off_end {
                best_off_end = s + h;
                arg_off = d;
            }
        }
        let (mut best_on_end, mut arg_on) = (f64::NEG_INFINITY, 0usize);
        for (d, (&s, &h)) in self.on.iter().zip(&self.on_end).enumerate() {
            if s + h > best_on_end {
                best_on_end = s + h;
                arg_on = d;
            }
        }
        let mut on = [NEG; ON_AGES];
        on[0] = best_off_end + e_on;
        for (d, x) in on.iter_mut().enumerate().take(ON_AGES - 1).skip(1) {
            *x = self.on[d - 1] + self.on_stay[d - 1] + e_on;
        }
        let from_prev = self.on[ON_AGES - 2] + self.on_stay[ON_AGES - 2];
        let from_self = self.on[ON_AGES - 1] + self.on_stay[ON_AGES - 1];
        b.on_tail_self = from_self > from_prev;
        on[ON_AGES - 1] = from_prev.max(from_self) + e_on;
        let mut off = [NEG; OFF_AGES];
        off[0] = best_on_end + e_off;
        for (d, x) in off.iter_mut().enumerate().take(OFF_AGES - 1).skip(1) {
            *x = self.off[d - 1] + self.off_stay[d - 1] + e_off;
        }
        let from_prev = self.off[OFF_AGES - 2] + self.off_stay[OFF_AGES - 2];
        let from_self = self.off[OFF_AGES - 1] + self.off_stay[OFF_AGES - 1];
        b.off_tail_self = from_self > from_prev;
        off[OFF_AGES - 1] = from_prev.max(from_self) + e_off;
        b.on0_from = arg_off as u8;
        b.off0_from = arg_on as u8;
        let top = on.iter().chain(off.iter()).fold(f64::NEG_INFINITY, |a, &x| a.max(x));
        for x in on.iter_mut().chain(off.iter_mut()) {
            *x = (*x - top).max(NEG);
        }
        self.on = on;
        self.off = off;
        self.back.push_back(b);
    }

    /// Il giudizio coerente dei passi da `a` (compreso) all'ultimo, a pezzi.
    fn segment_llr(&self, a: u64, j: &CohJudge) -> f64 {
        let b = self.t;
        let n = b.saturating_sub(a) as usize;
        if n == 0 || n >= CUM_LEN {
            return 0.0;
        }
        let k = n.div_ceil(j.piece.max(1)).clamp(1, j.max_pieces.max(1));
        let at = |i: u64| self.cum[(i as usize) % CUM_LEN];
        let mut pieces = [(0usize, 0.0f64); 64];
        let k = k.min(pieces.len());
        for (q, pc) in pieces.iter_mut().enumerate().take(k) {
            let pa = a + (q * n / k) as u64;
            let pb = a + ((q + 1) * n / k) as u64;
            let (x, y) = (at(pb), at(pa));
            let (re, im) = (x.re - y.re, x.im - y.im);
            *pc = ((pb - pa) as usize, (re * re + im * im).sqrt());
        }
        coherent_llr(&pieces[..k], j)
    }

    /// Un passo dell'ibrido coerente: `y` e' il campione di banda base (gia' in fase e riferito al rumore); un tono si giudica con la
    /// somma in fase dei suoi passi, le pause valgono zero. A cancello chiuso non si aprono toni.
    fn step_coherent(&mut self, y: Complex32, gate: bool, j: &CohJudge) {
        let last = self.cum[(self.t as usize) % CUM_LEN];
        self.t += 1;
        self.cum[(self.t as usize) % CUM_LEN] = Complex64 { re: last.re + f64::from(y.re), im: last.im + f64::from(y.im) };
        let mut b = Back::default();
        let (mut best_off_end, mut arg_off) = (f64::NEG_INFINITY, 0usize);
        for (d, (&s, &h)) in self.off.iter().zip(&self.off_end).enumerate() {
            if s + h > best_off_end {
                best_off_end = s + h;
                arg_off = d;
            }
        }
        let (mut best_on_end, mut arg_on) = (f64::NEG_INFINITY, 0usize);
        for (d, (&s, &h)) in self.on.iter().zip(&self.on_end).enumerate() {
            if s + h > best_on_end {
                best_on_end = s + h;
                arg_on = d;
            }
        }
        // somma progressiva della permanenza: il tono di eta' i (i+1 passi) e' rimasto i volte
        let mut stay_cum = [0.0f64; ON_AGES];
        for i in 1..ON_AGES {
            stay_cum[i] = stay_cum[i - 1] + self.on_stay[i - 1];
        }
        let now = self.t;
        let mut base = [NEG; ON_AGES];
        base[0] = if gate { best_off_end } else { best_off_end + j.closed_penalty };
        base[1..(ON_AGES - 1)].copy_from_slice(&self.base[..(ON_AGES - 2)]);
        let mut on = [NEG; ON_AGES];
        for i in 0..ON_AGES - 1 {
            if base[i] > NEG / 2.0 {
                on[i] = base[i] + stay_cum[i] + self.segment_llr(now - 1 - i as u64, j);
            }
        }
        // la coda: o arriva dall'ultima eta' o continua quella di prima
        let tail_stay = self.on_stay[ON_AGES - 1];
        let enter = self.base[ON_AGES - 2];
        let from_prev = if enter > NEG / 2.0 { enter + stay_cum[ON_AGES - 1] + self.segment_llr(now - ON_AGES as u64, j) } else { NEG };
        let (tb, ts, te) = self.tail;
        let from_self = if tb > NEG / 2.0 { tb + stay_cum[ON_AGES - 1] + f64::from(te + 1) * tail_stay + self.segment_llr(ts, j) } else { NEG };
        b.on_tail_self = from_self > from_prev;
        let (mut tail, tail_score) = if b.on_tail_self { ((tb, ts, te + 1), from_self) } else { ((enter, now - ON_AGES as u64, 0), from_prev) };
        on[ON_AGES - 1] = tail_score;
        let mut off = [NEG; OFF_AGES];
        off[0] = best_on_end;
        for (d, x) in off.iter_mut().enumerate().take(OFF_AGES - 1).skip(1) {
            *x = self.off[d - 1] + self.off_stay[d - 1];
        }
        let fp = self.off[OFF_AGES - 2] + self.off_stay[OFF_AGES - 2];
        let fs = self.off[OFF_AGES - 1] + self.off_stay[OFF_AGES - 1];
        b.off_tail_self = fs > fp;
        off[OFF_AGES - 1] = fp.max(fs);
        b.on0_from = arg_off as u8;
        b.off0_from = arg_on as u8;
        let top = on.iter().chain(off.iter()).fold(f64::NEG_INFINITY, |a, &x| a.max(x));
        for x in on.iter_mut().chain(off.iter_mut()).chain(base.iter_mut()) {
            *x = (*x - top).max(NEG);
        }
        tail.0 = (tail.0 - top).max(NEG);
        self.base = base;
        self.tail = tail;
        self.on = on;
        self.off = off;
        self.back.push_back(b);
    }

    /// Le etichette (acceso/spento) di tutti i passi in storia, dal piu' vecchio, lungo il percorso migliore.
    fn traceback(&self) -> Vec<bool> {
        let n = self.back.len();
        let mut labels = vec![false; n];
        let (mut is_on, mut age) = {
            let (bo, io) = self.on.iter().enumerate().fold((f64::NEG_INFINITY, 0), |a, (i, &x)| if x > a.0 { (x, i) } else { a });
            let (bf, jf) = self.off.iter().enumerate().fold((f64::NEG_INFINITY, 0), |a, (i, &x)| if x > a.0 { (x, i) } else { a });
            if bo > bf { (true, io) } else { (false, jf) }
        };
        for t in (0..n).rev() {
            labels[t] = is_on;
            let b = self.back[t];
            if is_on {
                if age == 0 {
                    is_on = false;
                    age = usize::from(b.on0_from);
                } else if age == ON_AGES - 1 && b.on_tail_self {
                } else {
                    age -= 1;
                }
            } else if age == 0 {
                is_on = true;
                age = usize::from(b.off0_from);
            } else if age == OFF_AGES - 1 && b.off_tail_self {
            } else {
                age -= 1;
            }
        }
        labels
    }
}

// ---------------------------------------------------------------- stadio 6: Morse

const MORSE: [(&str, &str); 46] = [
    (".-", "A"),
    ("-...", "B"),
    ("-.-.", "C"),
    ("-..", "D"),
    (".", "E"),
    ("..-.", "F"),
    ("--.", "G"),
    ("....", "H"),
    ("..", "I"),
    (".---", "J"),
    ("-.-", "K"),
    (".-..", "L"),
    ("--", "M"),
    ("-.", "N"),
    ("---", "O"),
    (".--.", "P"),
    ("--.-", "Q"),
    (".-.", "R"),
    ("...", "S"),
    ("-", "T"),
    ("..-", "U"),
    ("...-", "V"),
    (".--", "W"),
    ("-..-", "X"),
    ("-.--", "Y"),
    ("--..", "Z"),
    ("-----", "0"),
    (".----", "1"),
    ("..---", "2"),
    ("...--", "3"),
    ("....-", "4"),
    (".....", "5"),
    ("-....", "6"),
    ("--...", "7"),
    ("---..", "8"),
    ("----.", "9"),
    ("-..-.", "/"),
    ("..--..", "?"),
    (".-.-.-", "."),
    ("--..--", ","),
    ("-...-", "="),
    (".-.-.", "+"),
    ("-.--.", "<KN>"),
    ("...-.-", "<SK>"),
    (".-...", "<AS>"),
    ("-....-", "-"),
];

fn morse_lookup(code: &str) -> Option<&'static str> {
    MORSE.iter().find(|(c, _)| *c == code).map(|(_, t)| *t)
}

/// Il testo di un codice Morse; se sconosciuto prova a dividerlo in due caratteri (dal centro verso i lati).
pub fn morse_to_text(code: &str) -> Option<String> {
    if code.is_empty() {
        return None;
    }
    if let Some(t) = morse_lookup(code) {
        return Some(t.to_string());
    }
    let n = code.len();
    let mid = n / 2;
    for off in 0..n {
        for cut in [mid + off, mid.wrapping_sub(off)] {
            if cut == 0 || cut >= n {
                continue;
            }
            if let (Some(a), Some(b)) = (morse_lookup(&code[..cut]), morse_lookup(&code[cut..])) {
                return Some(format!("{a}{b}"));
            }
        }
    }
    None
}

// ---------------------------------------------------------------- stadio 7: segmentazione delle parole

/// Categorie del grafo lessicale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordKind {
    Generic,
    Traffic,
    Report,
    Keyword,
    Qrl,
    Call,
}

const TRAFFIC: &[(&str, f64)] = &[
    ("K", 0.884),
    ("TU", 0.0522),
    ("TO", 0.0442),
    ("AR", 0.0361),
    ("73", 0.0321),
    ("I", 0.0321),
    ("OK", 0.03),
    ("ES", 0.0281),
    ("SK", 0.0281),
    ("A", 0.0241),
    ("SO", 0.0241),
    ("IN", 0.0201),
    ("IS", 0.0201),
    ("R", 0.0201),
    ("THE", 0.0201),
    ("U", 0.0201),
    ("ABT", 0.0161),
    ("NEW", 0.0161),
    ("YOU", 0.0161),
    ("ON", 0.0061),
    ("?", 0.3),
    ("=", 0.26),
    ("RRR", 0.012),
    ("BT", 0.012),
    ("HW", 0.012),
    ("NO", 0.012),
    ("QRM", 0.012),
    ("QRN", 0.012),
    ("GUD", 0.012),
    ("AND", 0.012),
    ("BUT", 0.012),
    ("KHZ", 0.012),
    ("##F", 0.012),
    ("##DB", 0.012),
    ("#TT", 0.012),
    ("#OO", 0.012),
    ("#5O", 0.012),
    ("STN", 0.012),
    ("SIG", 0.012),
    ("FREQ", 0.012),
    ("BAND", 0.012),
    ("DX", 0.003),
    ("PSE", 0.003),
    ("KN", 0.003),
    ("BK", 0.003),
    ("TNX", 0.003),
    ("TKS", 0.003),
    ("EE", 0.003),
    ("GM", 0.003),
    ("GE", 0.003),
    ("GA", 0.003),
    ("GN", 0.003),
    ("DR", 0.003),
    ("OM", 0.003),
    ("UR", 0.003),
    ("RST", 0.003),
    ("OP", 0.003),
    ("MY", 0.003),
    ("NAME", 0.003),
    ("QTH", 0.003),
    ("HR", 0.003),
    ("NR", 0.003),
    ("RIG", 0.003),
    ("ANT", 0.003),
    ("QSL", 0.003),
    ("CFM", 0.003),
    ("GL", 0.003),
    ("CU", 0.003),
    ("AGN", 0.003),
    ("RR", 0.003),
    ("HI", 0.003),
    ("QSY", 0.003),
    ("QRT", 0.003),
    ("FER", 0.003),
    ("QSO", 0.003),
    ("FB", 0.003),
    ("VY", 0.003),
    ("SRI", 0.003),
    ("QRU", 0.003),
    ("QSB", 0.003),
    ("NW", 0.003),
    ("WX", 0.003),
    ("CW", 0.003),
    ("EU", 0.003),
    ("NA", 0.003),
    ("CALL", 0.003),
    ("BURO", 0.003),
    ("VIA", 0.003),
];
const REPORTS: &[(&str, f64)] = &[
    ("5NN", 0.8),
    ("HNN", 0.05),
    ("SNN", 0.05),
    ("SENN", 0.05),
    ("ESNN", 0.05),
    ("IINN", 0.05),
    ("VNN", 0.05),
    ("FNN", 0.05),
    ("LNN", 0.05),
    ("BNN", 0.05),
    ("3#N", 0.05),
    ("4#N", 0.05),
    ("5#N", 0.05),
    ("3#9", 0.05),
    ("4#9", 0.05),
    ("5#9", 0.05),
    ("H#N", 0.05),
];
const KEYWORDS: &[(&str, f64)] = &[
    ("DE", 0.45),
    ("TEST", 0.10),
    ("CQ", 0.05),
    ("SS", 0.05),
    ("UP", 0.05),
    ("UP1", 0.05),
    ("UP2", 0.05),
    ("QRZ", 0.05),
    ("QRZ?", 0.05),
    ("FD", 0.05),
    ("NA", 0.05),
];
const QRL: &[(&str, f64)] = &[("QRL?", 1.0), ("QRL", 1.0)];
/// Schemi generici per lunghezza (`*` = qualunque carattere).
const GENERIC_LEN: [f64; 12] = [0.1672, 0.2569, 0.1939, 0.1745, 0.0921, 0.025, 0.008, 0.003, 0.001, 0.0001, 0.0003, 0.0002];
/// Struttura del nominativo: primo pezzo, cifre, lettere finali, suffissi dopo la barra.
const CALL_PREFIX: &[(&str, f64)] = &[("@", 0.1997), ("@@", 0.744), ("#@", 0.055), ("#@@", 0.0013)];
const CALL_DIGITS: [f64; 4] = [0.9397, 0.053, 0.005, 0.0023];
const CALL_LETTERS: [f64; 6] = [0.0214, 0.3929, 0.5718, 0.0096, 0.003, 0.0013];
const CALL_SUFFIX: &[(&str, f64)] = &[("P", 0.4391), ("#", 0.2726), ("M", 0.1172), ("QRP", 0.0674), ("MM", 0.0498), ("LH", 0.0156), ("A", 0.007), ("AM", 0.007)];
/// Fattori delle categorie.
const KIND_W: [f64; 6] = [0.01, 0.3, 0.6, 0.6, 0.3, 0.4];

/// Penalita' (log) di un carattere che passa da un jolly.
fn wild(c: u8, pat: u8) -> Option<f64> {
    match pat {
        b'#' if c.is_ascii_digit() => Some((0.1f64).ln()),
        b'@' if c.is_ascii_uppercase() => Some((1.0f64 / 26.0).ln()),
        b'*' => Some((1.0f64 / 40.0).ln()),
        p if p == c => Some(0.0),
        _ => None,
    }
}

fn match_pattern(w: &[u8], pat: &[u8]) -> Option<f64> {
    if w.len() != pat.len() {
        return None;
    }
    let mut s = 0.0;
    for (&c, &p) in w.iter().zip(pat) {
        s += wild(c, p)?;
    }
    Some(s)
}

fn table_score(w: &[u8], table: &[(&str, f64)]) -> Option<f64> {
    let tot: f64 = table.iter().map(|x| x.1).sum();
    table.iter().filter_map(|(p, wt)| match_pattern(w, p.as_bytes()).map(|s| s + (wt / tot).ln())).fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x))))
}

fn letters(w: &[u8]) -> usize {
    w.iter().take_while(|c| c.is_ascii_uppercase()).count()
}

/// Punteggio (log) della struttura di nominativo: primo pezzo, cifre, lettere, eventuale barra e suffisso.
fn call_score(w: &[u8]) -> Option<f64> {
    let (body, suf) = match w.iter().position(|&c| c == b'/') {
        Some(k) => (&w[..k], Some(&w[k + 1..])),
        None => (w, None),
    };
    let ptot: f64 = CALL_PREFIX.iter().map(|x| x.1).sum();
    let mut best: Option<f64> = None;
    for (p, pw) in CALL_PREFIX {
        let pb = p.as_bytes();
        if body.len() <= pb.len() {
            continue;
        }
        let Some(ps) = match_pattern(&body[..pb.len()], pb) else { continue };
        let rest = &body[pb.len()..];
        let nd = rest.iter().take_while(|c| c.is_ascii_digit()).count();
        if nd == 0 || nd > CALL_DIGITS.len() {
            continue;
        }
        let tail = &rest[nd..];
        let nl = letters(tail);
        if nl != tail.len() || nl == 0 || nl > CALL_LETTERS.len() {
            continue;
        }
        let s = ps + (pw / ptot).ln() + CALL_DIGITS[nd - 1].ln() + nd as f64 * (0.1f64).ln() + CALL_LETTERS[nl - 1].ln() + nl as f64 * (1.0f64 / 26.0).ln();
        best = Some(best.map_or(s, |b: f64| b.max(s)));
    }
    let s = best?;
    match suf {
        None => Some(s),
        Some(x) => table_score(x, CALL_SUFFIX).map(|t| s + t + (0.03f64).ln()),
    }
}

/// Punteggio lessicale (log) di una parola e la sua categoria migliore.
pub fn word_score(w: &[u8]) -> (f64, WordKind) {
    let mut best = (f64::NEG_INFINITY, WordKind::Generic);
    let mut consider = |s: Option<f64>, k: WordKind, idx: usize| {
        if let Some(s) = s {
            let s = s + KIND_W[idx].ln();
            if s > best.0 {
                best = (s, k);
            }
        }
    };
    consider(table_score(w, TRAFFIC), WordKind::Traffic, 1);
    consider(table_score(w, REPORTS), WordKind::Report, 2);
    consider(table_score(w, KEYWORDS), WordKind::Keyword, 3);
    consider(table_score(w, QRL), WordKind::Qrl, 4);
    consider(call_score(w), WordKind::Call, 5);
    let n = w.len();
    let g = if n >= 1 && n <= GENERIC_LEN.len() { Some(GENERIC_LEN[n - 1].ln() + n as f64 * (1.0f64 / 40.0).ln()) } else { Some(-40.0 - 4.0 * n as f64) };
    consider(g, WordKind::Generic, 0);
    best
}

/// Segmentazione delle parole: tiene le lettere ancora in attesa e decide gli spazi con il grafo lessicale e le pause misurate.
#[derive(Clone)]
struct Segmenter {
    /// Lettere della parola in corso gia' stampate (contesto a sinistra).
    word: Vec<u8>,
    /// Lettere in attesa, ognuna con la probabilita' di pausa fra le parole dopo di lei (se gia' nota).
    pending: VecDeque<(u8, Option<f64>)>,
    lookahead: usize,
    lex_weight: f64,
}

impl Segmenter {
    fn new(pr: &ChainParams) -> Self {
        Segmenter { word: Vec::new(), pending: VecDeque::new(), lookahead: pr.seg_lookahead, lex_weight: pr.lex_weight }
    }

    fn push_char(&mut self, c: u8, out: &mut Vec<u8>) {
        if self.lookahead == 0 {
            self.word.push(c);
            out.push(c);
            return;
        }
        // una lettera senza pausa nota prima della successiva: nessun confine
        if let Some(last) = self.pending.back_mut()
            && last.1.is_none()
        {
            last.1 = Some(0.0);
        }
        self.pending.push_back((c, None));
        self.decide(false, out);
    }

    fn set_gap(&mut self, pw: f64, out: &mut Vec<u8>) {
        if self.lookahead == 0 {
            if pw > 0.5 && !self.word.is_empty() {
                out.push(b' ');
                self.word.clear();
            }
            return;
        }
        if let Some(last) = self.pending.back_mut()
            && last.1.is_none()
        {
            last.1 = Some(pw);
        }
        self.decide(false, out);
    }

    fn flush(&mut self, out: &mut Vec<u8>) {
        for x in self.pending.iter_mut().filter(|x| x.1.is_none()) {
            x.1 = Some(1.0);
        }
        self.decide(true, out);
        if !self.word.is_empty() {
            out.push(b' ');
            self.word.clear();
        }
    }

    /// Decide lo spazio dopo la lettera piu' vecchia in attesa quando ne ha abbastanza dopo (o alla fine).
    fn decide(&mut self, all: bool, out: &mut Vec<u8>) {
        loop {
            let known = self.pending.iter().take_while(|x| x.1.is_some()).count();
            if known == 0 || (!all && known <= self.lookahead) {
                return;
            }
            let space = self.best_boundary(known);
            let Some((c, pw)) = self.pending.pop_front() else { return };
            out.push(c);
            self.word.push(c);
            let pw = pw.unwrap_or(0.0);
            let sp = if all && self.pending.iter().all(|x| x.1.is_none()) && known == 1 { pw > 0.5 } else { space };
            if sp {
                out.push(b' ');
                self.word.clear();
            }
        }
    }

    /// Percorso migliore sui confini delle prime `known` lettere in attesa (la parola in corso a sinistra resta fissa):
    /// `true` se dopo la prima lettera in attesa c'e' un confine.
    fn best_boundary(&self, known: usize) -> bool {
        let chars: Vec<u8> = self.word.iter().copied().chain(self.pending.iter().take(known).map(|x| x.0)).collect();
        let gaps: Vec<f64> = self.pending.iter().take(known).map(|x| x.1.unwrap_or(0.0).clamp(1e-3, 1.0 - 1e-3)).collect();
        let w0 = self.word.len();
        let n = chars.len();
        // best[i]: miglior punteggio con un confine dopo il carattere i-1 (i = w0..=n), first[i]: se il primo confine dopo w0 e' a w0+1
        let mut best = vec![f64::NEG_INFINITY; n + 1];
        let mut first = vec![false; n + 1];
        best[0] = 0.0;
        for i in (w0 + 1)..=n {
            // una parola deve cominciare a 0 (contesto a sinistra) o dopo un confine in attesa
            let starts: Vec<usize> = std::iter::once(0).chain((w0 + 1)..i).collect();
            for &j in &starts {
                if j > 0 && j < w0 + 1 {
                    continue;
                }
                if !best[j].is_finite() || i - j > 14 {
                    continue;
                }
                let (lex, _) = word_score(&chars[j..i]);
                let mut s = best[j] + self.lex_weight * lex;
                // pause interne alla parola: niente confine
                for k in j.max(w0)..i.saturating_sub(1) {
                    s += (1.0 - gaps[k - w0]).ln();
                }
                if i > w0 {
                    s += gaps[i - 1 - w0].ln();
                }
                if s > best[i] {
                    best[i] = s;
                    first[i] = if j == 0 { i == w0 + 1 } else { first[j] };
                }
            }
        }
        // l'ultima pausa nota non obbliga: il confine finale e' libero (si toglie il suo termine se non e' confine)
        first[n]
    }
}

/// Consolidamento dei nominativi ripetuti: candidati con il numero di volte che sono stati letti (simili = al massimo una lettera
/// diversa a parita' di lunghezza, o uno contenuto nell'altro con una o due lettere in piu').
#[derive(Clone, Default, Debug)]
pub struct CallTally {
    pub calls: Vec<(String, u32)>,
}

fn similar(a: &str, b: &str) -> bool {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    if x.len() == y.len() {
        return x.iter().zip(y).filter(|(p, q)| p != q).count() <= 1;
    }
    let (s, l) = if x.len() < y.len() { (a, b) } else { (b, a) };
    let diff = l.len() - s.len();
    diff <= 2 && s.len() >= 3 && l.contains(s)
}

impl CallTally {
    /// Una parola letta: se ha la forma del nominativo la conta (sul candidato simile gia' visto, se c'e').
    pub fn offer(&mut self, w: &str) {
        if !matches!(word_score(w.as_bytes()).1, WordKind::Call) {
            return;
        }
        if let Some(e) = self.calls.iter_mut().find(|e| e.0 == w) {
            e.1 += 1;
            return;
        }
        if let Some(e) = self.calls.iter_mut().find(|e| similar(&e.0, w)) {
            e.1 += 1;
            return;
        }
        self.calls.push((w.to_string(), 1));
        if self.calls.len() > 64 {
            self.calls.remove(0);
        }
    }

    /// I candidati letti almeno `min` volte.
    pub fn confirmed(&self, min: u32) -> Vec<&str> {
        self.calls.iter().filter(|e| e.1 >= min).map(|e| e.0.as_str()).collect()
    }
}

// ---------------------------------------------------------------- un canale: stadi 2-7

/// Il ricevitore di un canale: dal confronto di forma al testo.
#[derive(Clone)]
pub struct ChannelChain {
    pr: ChainParams,
    tracker: Tracker,
    model: DurationModel,
    seq: Sequence,
    since_decide: usize,
    /// Run in corso: acceso/spento e passi.
    run_on: bool,
    run_len: usize,
    started: bool,
    /// Pausa in corso: carattere gia' chiuso, spazio gia' deciso.
    char_closed: bool,
    word_closed: bool,
    code: String,
    startup: Option<Vec<(bool, usize)>>,
    seg: Segmenter,
    closed_steps: usize,
    tally: CallTally,
    word_buf: Vec<u8>,
    recent: VecDeque<(bool, usize)>,
    since_check: usize,
    /// Ibrido coerente: ampiezza al quadrato del tono e rumore per passo (riferiti al rumore della banda), velocita' di rotazione della
    /// fase (prodotto dei passi consecutivi mediato), fase accumulata, passo precedente, varianze delle somme.
    coh_amp2: f64,
    coh_noise: f64,
    coh_rot: (f64, f64),
    coh_phase: f64,
    coh_prev: Option<Complex32>,
    coh_var: Vec<f64>,
    /// Potenza di ogni passo ancora da decidere (allineata alla storia della sequenza), quantile alto della potenza (dB) e passi
    /// accesi imparati dal percorso deciso.
    coh_pw: VecDeque<f64>,
    coh_q_hi: f64,
    coh_learned: u32,
    /// Elementi riconosciuti dall'ultimo controllo (per il ciclo di vita dei canali).
    pub elements: u32,
}

impl ChannelChain {
    pub fn new(wpm_hint: f64, pr: &ChainParams) -> Self {
        let model = DurationModel::new(wpm_hint, pr);
        ChannelChain {
            seq: Sequence::new(&model),
            tracker: Tracker::new(pr),
            model,
            since_decide: 0,
            run_on: false,
            run_len: 0,
            started: false,
            char_closed: true,
            word_closed: true,
            code: String::new(),
            startup: Some(Vec::new()),
            seg: Segmenter::new(pr),
            closed_steps: 0,
            tally: CallTally::default(),
            word_buf: Vec::new(),
            recent: VecDeque::with_capacity(STARTUP_ELEMENTS + 1),
            since_check: 0,
            coh_amp2: 0.0,
            coh_noise: 1.0,
            coh_rot: (0.0, 0.0),
            coh_phase: 0.0,
            coh_prev: None,
            coh_pw: VecDeque::new(),
            coh_q_hi: 0.0,
            coh_learned: 0,
            coh_var: if pr.emit_mode == 2 { coherent_var_sum(&chain_window(pr.iq_fc), CUM_LEN) } else { Vec::new() },
            elements: 0,
            pr: pr.clone(),
        }
    }

    pub fn wpm(&self) -> f64 {
        self.model.wpm()
    }

    pub fn gate(&self) -> bool {
        self.tracker.gate
    }

    /// Rapporto segnale/rumore (dB in 500 Hz) dei passi a tasto sicuramente abbassato.
    pub fn snr_db(&self) -> f32 {
        let enbw = 39.1;
        (10.0 * ((self.tracker.msq - 1.0).max(1e-3)).log10() + 10.0 * (enbw / 500.0f64).log10()) as f32
    }

    pub fn calls(&self) -> &CallTally {
        &self.tally
    }

    /// Un passo: il confronto di forma del tono e (per l'ibrido coerente) il campione di banda base; i caratteri nuovi in `out`.
    pub fn step(&mut self, shape: Shape, iq: Option<Complex32>, out: &mut Vec<u8>) -> KeyProb {
        let kp = self.tracker.step(shape, &self.pr);
        match iq {
            Some(y) if self.pr.emit_mode == 2 => self.step_coherent(y, kp),
            _ => self.seq.step(kp.p, self.pr.emit_weight),
        }
        self.since_decide += 1;
        self.closed_steps = if kp.gate { 0 } else { self.closed_steps + 1 };
        let start = out.len();
        if self.closed_steps == self.pr.flush_steps {
            let labels = self.seq.traceback();
            self.seq.reset();
            self.since_decide = 0;
            self.learn_levels(&labels, labels.len());
            self.commit(&labels, out);
            self.finish(out);
        } else if self.since_decide >= DECIDE && self.seq.back.len() >= HISTORY {
            self.since_decide = 0;
            let labels = self.seq.traceback();
            let n = labels.len() - DECIDE;
            self.seq.back.drain(..n);
            self.learn_levels(&labels, n);
            self.commit(&labels[..n], out);
        }
        self.note_words(start, out);
        kp
    }

    /// Il passo dell'ibrido coerente: in fase, rumore e ampiezza, poi la sequenza a durate esplicite con il giudizio coerente dei toni.
    fn step_coherent(&mut self, y: Complex32, kp: KeyProb) {
        // la velocita' di rotazione residua (tono non al centro della riga) dai passi consecutivi a tasto sicuramente abbassato
        if let Some(prev) = self.coh_prev
            && kp.p > 0.8
        {
            let (re, im) = (f64::from(y.re * prev.re + y.im * prev.im), f64::from(y.im * prev.re - y.re * prev.im));
            self.coh_rot.0 += 0.05 * (re - self.coh_rot.0);
            self.coh_rot.1 += 0.05 * (im - self.coh_rot.1);
        }
        self.coh_prev = Some(y);
        let rate = if self.coh_rot.0 != 0.0 || self.coh_rot.1 != 0.0 { self.coh_rot.1.atan2(self.coh_rot.0) } else { 0.0 };
        self.coh_phase = (self.coh_phase + rate) % (2.0 * PI);
        let (sn, cs) = self.coh_phase.sin_cos();
        let (yr, yi) = (f64::from(y.re), f64::from(y.im));
        let z = Complex32::new((yr * cs + yi * sn) as f32, (yi * cs - yr * sn) as f32);
        let pw = f64::from(z.norm_sqr());
        self.coh_pw.push_back(pw);
        if !kp.gate {
            self.coh_noise += 0.02 * (pw - self.coh_noise);
        }
        // finche' il percorso deciso non ha insegnato l'ampiezza: quantile 90% della potenza
        let l = 10.0 * pw.max(1e-9).log10();
        self.coh_q_hi += 0.05 * (0.9 - if l < self.coh_q_hi { 1.0 } else { 0.0 });
        if l > self.coh_q_hi + 3.0 {
            self.coh_q_hi += 0.2 * (l - self.coh_q_hi);
        }
        if self.coh_learned < 20 {
            self.coh_amp2 = (10f64.powf(self.coh_q_hi / 10.0) - self.coh_noise).max(0.0);
        }
        let noise = self.coh_noise.max(0.3);
        let amp2 = (self.coh_amp2 / noise).max(self.pr.coh_min);
        let nu = (amp2 / self.pr.coh_snr_cap).max(1.0);
        let piece = ((self.pr.coh_piece_units * self.model.mu[0].exp()).round() as usize).max(1);
        let scale = (1.0 / noise.sqrt()) as f32;
        let judge = CohJudge { amp2, nu, piece, max_pieces: self.pr.coh_max_pieces, closed_penalty: -self.pr.coh_closed, var_sum: &self.coh_var };
        self.seq.step_coherent(z * scale, kp.gate, &judge);
    }

    /// Ibrido coerente: il percorso deciso insegna ampiezza (passi accesi lontani dai fronti) e rumore (pause); si tolgono le potenze
    /// dei primi `n` passi.
    fn learn_levels(&mut self, labels: &[bool], n: usize) {
        if self.pr.emit_mode != 2 {
            return;
        }
        let k = n.min(self.coh_pw.len());
        for i in 0..k {
            let inner = i > 0 && i + 1 < labels.len() && labels[i - 1] == labels[i] && labels[i + 1] == labels[i];
            let pw = self.coh_pw[i];
            if inner && labels[i] {
                let a = if self.coh_learned < 20 { 0.2 } else { 0.03 };
                self.coh_amp2 += a * ((pw - self.coh_noise).max(0.0) - self.coh_amp2);
                self.coh_learned = self.coh_learned.saturating_add(1);
            } else if inner && self.tracker.gate {
                self.coh_noise += 0.01 * (pw - self.coh_noise);
            }
        }
        self.coh_pw.drain(..k);
        if labels.len() == n {
            self.coh_pw.clear();
        }
    }

    fn note_words(&mut self, start: usize, out: &[u8]) {
        for &c in &out[start..] {
            if c == b' ' {
                if !self.word_buf.is_empty() {
                    let w = String::from_utf8_lossy(&self.word_buf).into_owned();
                    self.tally.offer(&w);
                    self.word_buf.clear();
                }
            } else {
                self.word_buf.push(c);
            }
        }
    }

    fn commit(&mut self, labels: &[bool], out: &mut Vec<u8>) {
        for &l in labels {
            if !self.started {
                if l {
                    self.started = true;
                    self.run_on = true;
                    self.run_len = 1;
                }
                continue;
            }
            if l == self.run_on {
                self.run_len += 1;
            } else {
                self.element(self.run_on, self.run_len, out);
                self.run_on = l;
                self.run_len = 1;
                if !l {
                    self.char_closed = false;
                    self.word_closed = false;
                }
            }
        }
        // pausa in corso: chiude il carattere e decide lo spazio appena la durata lo dice
        if self.started && !self.run_on && self.startup.is_none() {
            let (c, pw) = self.model.classify_off(self.run_len as f64);
            if c >= 3 && !self.char_closed {
                self.char_closed = true;
                self.close_char(out);
            }
            if c == 4 && !self.word_closed {
                self.word_closed = true;
                self.seg.set_gap(pw.max(0.99), out);
            }
        }
    }

    /// Fine della trasmissione: chiude il carattere e la parola in corso e riparte.
    fn finish(&mut self, out: &mut Vec<u8>) {
        if let Some(buf) = self.startup.take() {
            if buf.len() >= 4 {
                self.run_startup(buf, out);
            } else {
                self.startup = Some(Vec::new());
                self.code.clear();
            }
        }
        if self.started && self.run_on {
            self.element(true, self.run_len, out);
        }
        if self.startup.is_none() {
            if !self.char_closed {
                self.close_char(out);
            }
            if !self.word_closed {
                self.seg.set_gap(1.0, out);
            }
        }
        self.seg.flush(out);
        self.started = false;
        self.run_on = false;
        self.run_len = 0;
        self.char_closed = true;
        self.word_closed = true;
    }

    fn element(&mut self, on: bool, len: usize, out: &mut Vec<u8>) {
        self.elements = self.elements.saturating_add(1);
        if let Some(buf) = self.startup.as_mut() {
            buf.push((on, len));
            if buf.len() >= STARTUP_ELEMENTS {
                let buf = std::mem::take(buf);
                self.startup = None;
                self.run_startup(buf, out);
            }
            return;
        }
        self.classified(on, len, out);
    }

    fn run_startup(&mut self, buf: Vec<(bool, usize)>, out: &mut Vec<u8>) {
        let on: Vec<f64> = buf.iter().filter(|e| e.0).map(|e| e.1 as f64).collect();
        let off: Vec<f64> = buf.iter().filter(|e| !e.0).map(|e| e.1 as f64).collect();
        let mu = startup_means(&on, &off, self.model.mu[0]);
        self.model.restart(mu, &self.pr);
        self.startup = None;
        for (o, l) in buf {
            self.classified(o, l, out);
        }
    }

    fn classified(&mut self, on: bool, len: usize, out: &mut Vec<u8>) {
        let d = len as f64;
        self.recent.push_back((on, len));
        if self.recent.len() > STARTUP_ELEMENTS {
            self.recent.pop_front();
        }
        self.since_check += 1;
        if self.since_check >= STARTUP_ELEMENTS && self.recent.len() >= STARTUP_ELEMENTS && self.pr.recheck_margin > 0.0 {
            // controllo periodico: se le medie rifatte sugli ultimi elementi li spiegano molto meglio, si riparte da quelle
            self.since_check = 0;
            let on: Vec<f64> = self.recent.iter().filter(|e| e.0).map(|e| e.1 as f64).collect();
            let off: Vec<f64> = self.recent.iter().filter(|e| !e.0).map(|e| e.1 as f64).collect();
            let mu = startup_means(&on, &off, self.model.mu[0]);
            let mut alt = self.model.clone();
            alt.mu = mu;
            if alt.fit(&self.recent) > self.model.fit(&self.recent) + self.pr.recheck_margin {
                self.model.restart(mu, &self.pr);
            }
        }
        if on {
            let c = self.model.classify_on(d);
            self.code.push(if c == 0 { '.' } else { '-' });
            self.model.update(c, d);
            if self.code.len() > self.pr.restart_len {
                // un carattere cosi' lungo non esiste: la velocita' e' cambiata, si rifa' l'avvio sugli ultimi elementi
                let on: Vec<f64> = self.recent.iter().filter(|e| e.0).map(|e| e.1 as f64).collect();
                let off: Vec<f64> = self.recent.iter().filter(|e| !e.0).map(|e| e.1 as f64).collect();
                let mu = startup_means(&on, &off, self.model.mu[0]);
                self.model.restart(mu, &self.pr);
                self.code.clear();
            }
        } else {
            let (c, pw) = self.model.classify_off(d);
            if c >= 3 {
                self.close_char(out);
                self.seg.set_gap(if c == 4 { pw.max(0.5) } else { pw.min(0.5) }, out);
            }
            if c < 4 || d.ln() < LN_MAX {
                self.model.update(c, d);
            }
            self.char_closed = true;
            self.word_closed = true;
        }
        self.seq.retable(&self.model);
    }

    fn close_char(&mut self, out: &mut Vec<u8>) {
        if self.code.is_empty() {
            return;
        }
        if let Some(t) = morse_to_text(&self.code) {
            for c in t.bytes() {
                self.seg.push_char(c, out);
            }
        }
        self.code.clear();
    }
}

// ---------------------------------------------------------------- canale singolo

/// Il ricevitore a catena su un tono noto, con la stessa interfaccia di base di [`crate::cw::CwDemodulator`].
pub struct CwChain {
    spec: Spectrum,
    center: usize,
    ch: ChannelChain,
}

impl CwChain {
    pub fn new(tone_hz: f32, wpm_hint: f32) -> Self {
        Self::with_params(tone_hz, wpm_hint, &ChainParams::default())
    }

    pub fn with_params(tone_hz: f32, wpm_hint: f32, pr: &ChainParams) -> Self {
        // il tono si porta al centro di una riga
        let center = (f64::from(tone_hz) / BIN_HZ).round().max(SHAPE_HALF as f64 + 1.0) as usize;
        let shift = center as f64 * BIN_HZ - f64::from(tone_hz);
        CwChain { spec: Spectrum::new(pr.window_fc, if pr.emit_mode == 2 { pr.iq_fc } else { 0.0 }, shift, 200.0, 3_500.0), center, ch: ChannelChain::new(f64::from(wpm_hint), pr) }
    }

    /// Altro audio a 12 kHz; restituisce i caratteri nuovi.
    pub fn process(&mut self, audio: &[f32]) -> Vec<u8> {
        let mut out = Vec::new();
        for &x in audio {
            if self.spec.push(x) {
                let s = match_shape(&self.spec.power, self.center, self.spec.noise);
                let iq = self.spec.baseband(self.center);
                self.ch.step(s, iq, &mut out);
            }
        }
        out
    }

    pub fn wpm(&self) -> f32 {
        self.ch.wpm() as f32
    }

    pub fn snr_db(&self) -> f32 {
        self.ch.snr_db()
    }

    pub fn calls(&self) -> &CallTally {
        self.ch.calls()
    }
}

// ---------------------------------------------------------------- stadio 8: BigScan

/// Stato di un canale di BigScan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelState {
    Learning,
    Active,
    Idle,
}

struct BandChannel {
    id: u32,
    bin: usize,
    home: usize,
    frac: f32,
    rx: ChannelChain,
    text: String,
    state: ChannelState,
    count: u32,
    last_gate: bool,
}

/// Un canale di BigScan come lo vede chi legge.
#[derive(Clone, Debug)]
pub struct ChainChannelInfo {
    pub id: u32,
    pub freq_hz: f32,
    pub wpm: f32,
    pub text: String,
    pub state: ChannelState,
}

/// BigScan CW con la catena: uno spettro per tutta la banda, canali aperti sui massimi stretti del contrasto locale.
pub struct CwChainBand {
    pr: ChainParams,
    spec: Spectrum,
    acc: Vec<f32>,
    contrast: Vec<f32>,
    lo: usize,
    hi: usize,
    history: VecDeque<Backlog>,
    channels: Vec<BandChannel>,
    max_channels: usize,
    next_id: u32,
    steps: u64,
    out: Vec<u8>,
}

/// Uno spettro tenuto per i canali nuovi.
struct Backlog {
    power: Vec<f32>,
    noise: f32,
    iq: Vec<Complex32>,
    count: u64,
    iq_noise: f32,
}

/// Spettri tenuti per i canali nuovi.
const BACKLOG: usize = 16;
/// Righe mascherate attorno a un canale aperto.
const MASK: usize = 5;

impl CwChainBand {
    pub fn new(lo_hz: f32, hi_hz: f32, max_channels: usize) -> Self {
        Self::with_params(lo_hz, hi_hz, max_channels, &ChainParams::default())
    }

    pub fn with_params(lo_hz: f32, hi_hz: f32, max_channels: usize, pr: &ChainParams) -> Self {
        let lo = ((f64::from(lo_hz) / BIN_HZ) as usize).max(SHAPE_HALF + 1);
        let hi = ((f64::from(hi_hz) / BIN_HZ) as usize).min(NFFT / 2 - SHAPE_HALF - 1).max(lo + 1);
        CwChainBand {
            spec: Spectrum::new(pr.window_fc, if pr.emit_mode == 2 { pr.iq_fc } else { 0.0 }, 0.0, f64::from(lo_hz), f64::from(hi_hz)),
            acc: vec![0.0; NFFT / 2],
            contrast: vec![0.0; NFFT / 2],
            lo,
            hi,
            history: VecDeque::with_capacity(BACKLOG + 1),
            channels: Vec::new(),
            max_channels,
            next_id: 1,
            steps: 0,
            out: Vec::new(),
            pr: pr.clone(),
        }
    }

    pub fn process(&mut self, audio: &[f32]) {
        for &x in audio {
            if self.spec.push(x) {
                self.step();
            }
        }
    }

    fn step(&mut self) {
        self.steps += 1;
        let noise = self.spec.noise.max(1e-30);
        let power = &self.spec.power;
        for (a, &p) in self.acc.iter_mut().zip(power) {
            *a = 0.97 * *a + p / noise;
        }
        for i in self.lo..self.hi {
            let side = self.acc[i - 4] + self.acc[i + 4];
            self.contrast[i] = 0.8 * self.contrast[i] + self.acc[i] / side.max(1e-12);
        }
        self.history.push_back(Backlog { power: power.clone(), noise, iq: self.spec.iq.clone(), count: self.spec.count, iq_noise: self.spec.iq_noise });
        if self.history.len() > BACKLOG {
            self.history.pop_front();
        }
        // canali aperti
        for ch in &mut self.channels {
            // inseguimento del centro: massimo stretto entro ±2 righe
            let mut off = 0i32;
            for k in [0i32, -1, 1, -2, 2] {
                let b = (ch.bin as i32 + k) as usize;
                if b > self.lo && b + 1 < self.hi && power[b] / noise > 7.5 && power[b] > power[b - 1] && power[b] > power[b + 1] {
                    off = k;
                    break;
                }
            }
            ch.frac = 0.97 * ch.frac + 0.03 * off as f32;
            if ch.frac.abs() >= 1.0 {
                let s = ch.frac.trunc();
                // il centro si sposta al massimo di due righe da dove il canale e' nato: non scivola verso una stazione vicina piu' forte
                let lo = (ch.home as i32 - 2).max(self.lo as i32);
                let hi = (ch.home as i32 + 2).min(self.hi as i32 - 1);
                ch.bin = (ch.bin as i32 + s as i32).clamp(lo, hi) as usize;
                ch.frac -= s;
            }
            let shape = match_shape(power, ch.bin, noise);
            let iq = baseband_of(&self.spec.iq, ch.bin, self.spec.count, self.spec.iq_noise);
            self.out.clear();
            let kp = ch.rx.step(shape, iq, &mut self.out);
            ch.text.push_str(&String::from_utf8_lossy(&self.out));
            if ch.text.len() > 500 {
                let cut = ch.text.len() - 375;
                ch.text.drain(..cut);
            }
            life(ch, kp.gate, !self.out.is_empty(), &self.pr);
        }
        self.channels.retain(|c| c.count != u32::MAX);
        // canali nuovi
        if self.steps <= 100 || self.channels.len() >= self.max_channels {
            return;
        }
        let mut opened = 0;
        for i in (self.lo + 1)..(self.hi - 1) {
            let c = self.contrast[i];
            if c <= self.pr.band_peak || c <= self.contrast[i - 1] || c <= self.contrast[i + 1] {
                continue;
            }
            // maschera attorno ai canali aperti, piu' larga attorno a quelli forti (la loro dispersione non apre canali falsi)
            let (wide, strong) = (self.pr.band_mask_strong, f64::from(self.pr.band_strong));
            if self.channels.iter().any(|ch| ch.bin.abs_diff(i) <= MASK || (ch.rx.tracker.msq > strong && ch.bin.abs_diff(i) <= wide)) {
                continue;
            }
            self.open(i);
            opened += 1;
            if opened >= 10 || self.channels.len() >= self.max_channels {
                break;
            }
        }
    }

    fn open(&mut self, bin: usize) {
        let mut rx = ChannelChain::new(25.0, &self.pr);
        let mut text = Vec::new();
        // i canali nuovi partono dagli spettri tenuti
        for h in &self.history {
            rx.step(match_shape(&h.power, bin, h.noise), baseband_of(&h.iq, bin, h.count, h.iq_noise), &mut text);
        }
        self.channels.push(BandChannel {
            id: self.next_id,
            bin,
            home: bin,
            frac: 0.0,
            rx,
            text: String::from_utf8_lossy(&text).into_owned(),
            state: ChannelState::Learning,
            count: 0,
            last_gate: false,
        });
        self.next_id += 1;
    }

    pub fn channels(&self) -> Vec<ChainChannelInfo> {
        self.channels
            .iter()
            .map(|c| ChainChannelInfo { id: c.id, freq_hz: (c.bin as f64 * BIN_HZ) as f32, wpm: c.rx.wpm() as f32, text: c.text.clone(), state: c.state })
            .collect()
    }
}

/// Ciclo di vita di un canale: in apprendimento si chiude se il cancello non cambia per 500 passi; attivo si chiude dopo 9000 passi
/// senza caratteri; inattivo (cancello chiuso) si chiude dopo `band_idle_steps` passi. `count = u32::MAX` = da chiudere.
fn life(ch: &mut BandChannel, gate: bool, printed: bool, pr: &ChainParams) {
    match ch.state {
        ChannelState::Learning => {
            if printed {
                ch.state = ChannelState::Active;
                ch.count = 0;
            } else if gate != ch.last_gate {
                ch.count = 0;
            } else {
                ch.count += 1;
                if ch.count > 500 {
                    ch.count = u32::MAX;
                }
            }
        }
        ChannelState::Active => {
            if !gate {
                ch.state = ChannelState::Idle;
                ch.count = 0;
            } else if printed {
                ch.count = 0;
            } else {
                ch.count += 1;
                if ch.count > 9000 {
                    ch.count = u32::MAX;
                }
            }
        }
        ChannelState::Idle => {
            if gate {
                ch.state = ChannelState::Active;
                ch.count = 0;
            } else {
                ch.count += 1;
                if ch.count > pr.band_idle_steps {
                    ch.count = u32::MAX;
                }
            }
        }
    }
    ch.last_gate = gate;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_audio(text: &[u8], tone: f32, wpm: f32, amp: f32) -> Vec<f32> {
        let mut a = vec![0.0f32; 12_000];
        a.extend(crate::cw::modulate(text, tone, wpm, amp));
        a.extend(vec![0.0f32; 24_000]);
        a
    }

    fn noise(a: &mut [f32], sigma: f32, seed: u64) {
        let mut s = seed | 1;
        let mut next = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 11) as f64 / (1u64 << 53) as f64
        };
        for x in a.iter_mut() {
            let (u, v) = (next().max(1e-12), next());
            *x += sigma * ((-2.0 * u.ln()).sqrt() * (2.0 * PI * v).cos()) as f32;
        }
    }

    #[test]
    fn finestra_simmetrica_e_passa_basso() {
        let w = chain_window(25.0);
        assert_eq!(w.len(), NFFT);
        assert!((w[100] - w[NFFT - 1 - 100]).abs() < 1e-6);
        let mid = w[NFFT / 2];
        assert!(mid > 0.9 && w[0].abs() < 1e-3);
    }

    #[test]
    fn spettro_a_cadenza_fissa() {
        let mut s = Spectrum::new(25.0, 0.0, 0.0, 200.0, 3_500.0);
        let n = (0..12_000).filter(|i| s.push((2.0 * PI * 703.125 * *i as f64 / RATE).sin() as f32)).count();
        assert_eq!(n, 12_000 / HOP);
        let peak = s.power.iter().enumerate().fold((0, 0.0f32), |a, (i, &p)| if p > a.1 { (i, p) } else { a }).0;
        assert_eq!(peak, 60);
    }

    #[test]
    fn forma_tono_e_rumore() {
        let mut p = vec![1.0f32; 64];
        p[30] = 100.0;
        let s = match_shape(&p, 30, 1.0);
        assert!((s.a - 10.0).abs() < 1e-3 && (s.n - 1.0).abs() < 1e-3 && (s.d - 9.0).abs() < 1e-3 && (s.side - 1.0).abs() < 1e-3);
        // un disturbo largo alza anche le spalle: il residuo cala
        let q = vec![50.0f32; 64];
        let s2 = match_shape(&q, 30, 1.0);
        assert!(s2.d < 0.0, "{s2:?}");
    }

    #[test]
    fn bessel_logaritmico() {
        assert!(ln_i0(0.0).abs() < 1e-6);
        assert!((ln_i0(1.0) - 1.266_065_9f64.ln()).abs() < 1e-5);
        assert!((ln_i0(10.0) - 2_815.716_6f64.ln()).abs() < 1e-4);
    }

    #[test]
    fn estremi_scorrevoli_e_media() {
        let mut mx = SlidingExt::new(3, true);
        let mut mn = SlidingExt::new(3, false);
        let v = [1.0, 5.0, 2.0, 0.0, 0.0, 0.0];
        let maxs: Vec<f32> = v.iter().map(|&x| mx.push(x)).collect();
        let mins: Vec<f32> = v.iter().map(|&x| mn.push(x)).collect();
        assert_eq!(maxs, vec![1.0, 5.0, 5.0, 5.0, 2.0, 0.0]);
        assert_eq!(mins, vec![1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
        let mut b = BoxMean::new(2);
        assert_eq!(b.push(2.0), 1.0);
        assert_eq!(b.push(4.0), 3.0);
        assert_eq!(b.push(0.0), 2.0);
        assert_eq!(median3(3.0, 1.0, 2.0), 2.0);
    }

    #[test]
    fn inviluppo_cancello_e_probabilita() {
        let pr = ChainParams::default();
        let mut t = Tracker::new(&pr);
        // solo rumore: cancello chiuso, probabilita' quasi nulla
        for _ in 0..200 {
            let k = t.step(Shape { a: 0.9, n: 0.6, d: 0.3, side: 1.0 }, &pr);
            assert!(!k.gate && k.p < 0.01);
        }
        // tono a meta' tempo: il cancello apre, tasto giu' ~1, tasto su ~0
        let mut last = (0.0, 0.0);
        for i in 0..400 {
            let on = (i / 6) % 2 == 0;
            let s = if on { Shape { a: 8.0, n: 1.0, d: 7.0, side: 1.0 } } else { Shape { a: 0.9, n: 0.6, d: 0.3, side: 1.0 } };
            let k = t.step(s, &pr);
            if i > 300 {
                if on { last.0 = k.p } else { last.1 = k.p }
            }
        }
        assert!(t.gate && last.0 > 0.6 && last.1 < 0.3, "{last:?}");
    }

    #[test]
    fn durate_classi_e_velocita() {
        let pr = ChainParams::default();
        let m = DurationModel::new(25.0, &pr);
        assert!((m.wpm() - 25.0).abs() < 3.0, "{}", m.wpm());
        let u = 112.5 / 25.0;
        assert_eq!(m.classify_on(u), 0);
        assert_eq!(m.classify_on(3.0 * u), 1);
        assert_eq!(m.classify_off(u).0, 2);
        assert_eq!(m.classify_off(3.0 * u).0, 3);
        assert_eq!(m.classify_off(12.0 * u).0, 4);
        // la coda lunga tiene le pause lunghissime fra le parole
        assert_eq!(m.classify_off(200.0).0, 4);
    }

    #[test]
    fn kalman_correlato_segue_la_velocita() {
        let pr = ChainParams::default();
        let mut m = DurationModel::new(20.0, &pr);
        let u = 112.5 / 35.0;
        for _ in 0..40 {
            m.update(0, u);
            m.update(2, u);
            m.update(1, 3.0 * u);
            m.update(3, 3.0 * u);
        }
        assert!((m.wpm() - 35.0).abs() < 4.0, "{}", m.wpm());
        // un punto osservato sposta anche la media delle linee
        let mut m2 = DurationModel::new(20.0, &pr);
        let before = m2.mu[1];
        m2.update(0, 2.0);
        assert!(m2.mu[1] < before);
    }

    #[test]
    fn avvio_trova_l_unita() {
        let u: f64 = 3.0;
        let on = [u, 3.0 * u, u, u, 3.0 * u, 3.0 * u, u];
        let off = [u, 3.0 * u, u, 7.0 * u, u, 3.0 * u, u];
        let mu = startup_means(&on, &off, (6.0f64).ln());
        assert!((mu[0].exp() - u).abs() < 0.6 && (mu[1].exp() - 3.0 * u).abs() < 1.5, "{mu:?}");
        // pause allungate e toni accorciati dal rivelatore: ognuno con la propria unita'
        let on2: Vec<f64> = on.iter().map(|d| d - 1.0).collect();
        let off2: Vec<f64> = off.iter().map(|d| d + 1.0).collect();
        let mu2 = startup_means(&on2, &off2, (6.0f64).ln());
        assert!((mu2[2].exp() - (u + 1.0)).abs() < 0.8 && (mu2[3].exp() - (3.0 * u + 1.0)).abs() < 2.0, "{mu2:?}");
    }

    #[test]
    fn rischi_della_coda_validi() {
        let pr = ChainParams::default();
        let m = DurationModel::new(25.0, &pr);
        let (s, e) = m.hazards(false, OFF_AGES);
        assert_eq!(s.len(), OFF_AGES);
        assert!(s.iter().chain(&e).all(|x| x.is_finite() && *x <= 0.0));
    }

    #[test]
    fn sequenza_ritrova_le_durate() {
        let pr = ChainParams::default();
        let m = DurationModel::new(25.0, &pr);
        let mut s = Sequence::new(&m);
        let mut truth = Vec::new();
        for (on, n) in [(false, 10), (true, 5), (false, 5), (true, 14), (false, 14), (true, 5), (false, 10)] {
            for _ in 0..n {
                s.step(if on { 0.9 } else { 0.1 }, 1.0);
                truth.push(on);
            }
        }
        assert_eq!(s.traceback(), truth);
    }

    #[test]
    fn morse_e_divisione() {
        assert_eq!(morse_to_text(".-").as_deref(), Some("A"));
        assert_eq!(morse_to_text("-.-.--.-").as_deref(), Some("CQ"));
        assert_eq!(morse_to_text(""), None);
    }

    #[test]
    fn lessico_categorie() {
        assert_eq!(word_score(b"5NN").1, WordKind::Report);
        assert_eq!(word_score(b"IU3ABC").1, WordKind::Call);
        assert_eq!(word_score(b"K1XYZ/P").1, WordKind::Call);
        assert_eq!(word_score(b"TU").1, WordKind::Traffic);
        assert_eq!(word_score(b"CQ").1, WordKind::Keyword);
        assert_eq!(word_score(b"QRL?").1, WordKind::Qrl);
        assert_eq!(word_score(b"XQZ").1, WordKind::Generic);
        // una parola sola vale piu' di due pezzi generici
        let (whole, _) = word_score(b"IU3ABC");
        let parts = word_score(b"IU3").0 + word_score(b"ABC").0;
        assert!(whole > parts);
    }

    #[test]
    fn segmentazione_unisce_un_nominativo_spezzato() {
        let pr = ChainParams { seg_lookahead: 3, ..ChainParams::default() };
        let mut s = Segmenter::new(&pr);
        let mut out = Vec::new();
        // pausa incerta (0,55) dentro il nominativo: il lessico tiene la parola intera
        for (c, pw) in [(b'I', 0.02), (b'U', 0.02), (b'3', 0.55), (b'A', 0.02), (b'B', 0.02), (b'C', 0.98), (b'T', 0.02), (b'U', 0.98)] {
            s.push_char(c, &mut out);
            s.set_gap(pw, &mut out);
        }
        s.flush(&mut out);
        assert_eq!(String::from_utf8_lossy(&out).trim(), "IU3ABC TU");
    }

    #[test]
    fn nominativi_ripetuti_si_consolidano() {
        let mut t = CallTally::default();
        for w in ["IU3ABC", "IU3ABC", "IU3ABE", "TU", "K1XYZ"] {
            t.offer(w);
        }
        assert_eq!(t.confirmed(3), vec!["IU3ABC"]);
        assert!(similar("K1XYZ", "K1XY") && !similar("K1XYZ", "DL2AB"));
    }

    #[test]
    fn canale_legge_un_testo_pulito() {
        let mut audio = tone_audio(b"CQ TEST IU3ABC IU3ABC K", 700.0, 25.0, 0.1);
        noise(&mut audio, 0.05, 4);
        let mut rx = CwChain::new(700.0, 20.0);
        let mut out = Vec::new();
        for b in audio.chunks(1200) {
            out.extend(rx.process(b));
        }
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("IU3ABC"), "{s:?}");
        assert!((rx.wpm() - 25.0).abs() < 4.0, "{}", rx.wpm());
    }

    #[test]
    fn canale_regge_un_segnale_fortissimo() {
        // circa 60 dB sopra il rumore: la dispersione del tono nelle spalle non deve riempire le pause
        let mut audio = tone_audio(b"CQ TEST IU3ABC IU3ABC K", 700.0, 32.0, 0.5);
        noise(&mut audio, 0.0005, 6);
        let mut rx = CwChain::new(700.0, 20.0);
        let out: Vec<u8> = audio.chunks(1200).flat_map(|b| rx.process(b)).collect();
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("IU3ABC"), "{s:?}");
    }

    #[test]
    fn banda_base_coerente_calibrata() {
        // sul solo rumore la varianza di una somma di n passi in fase e' quella prevista dalla finestra
        for fc in [60.0, 100.0] {
            let mut audio = vec![0.0f32; 12_000 * 60];
            noise(&mut audio, 0.05, 9);
            let mut sp = Spectrum::new(25.0, fc, 3.125, 200.0, 3_500.0);
            let mut ys = Vec::new();
            for &x in &audio {
                if sp.push(x) && sp.count > 40 {
                    ys.extend(sp.baseband(60));
                }
            }
            let var = coherent_var_sum(&chain_window(fc), 16);
            for n in [1usize, 4, 10] {
                let sums: Vec<f64> = ys
                    .chunks_exact(n)
                    .map(|c| {
                        let (re, im) = c.iter().fold((0.0f64, 0.0f64), |a, z| (a.0 + f64::from(z.re), a.1 + f64::from(z.im)));
                        re * re + im * im
                    })
                    .collect();
                let got = sums.iter().sum::<f64>() / sums.len() as f64;
                assert!((got / var[n] - 1.0).abs() < 0.15, "fc {fc} n {n}: misurata {got:.2} prevista {:.2}", var[n]);
            }
        }
    }

    #[test]
    fn ibrido_coerente_legge_e_tace_sul_rumore() {
        let pr = ChainParams { emit_mode: 2, ..ChainParams::default() };
        let mut audio = tone_audio(b"CQ TEST IU3ABC IU3ABC K", 700.0, 25.0, 0.1);
        noise(&mut audio, 0.05, 4);
        let mut rx = CwChain::with_params(700.0, 20.0, &pr);
        let out: Vec<u8> = audio.chunks(1200).flat_map(|b| rx.process(b)).collect();
        let s = String::from_utf8_lossy(&out);
        assert!(s.contains("IU3ABC"), "{s:?}");
        let mut quiet = vec![0.0f32; 12_000 * 15];
        noise(&mut quiet, 0.05, 3);
        let mut rx = CwChain::with_params(700.0, 20.0, &pr);
        let n: usize = quiet.chunks(1200).map(|b| rx.process(b).iter().filter(|c| **c != b' ').count()).sum();
        assert!(n <= 3, "{n}");
    }

    #[test]
    fn verosimiglianza_coerente_premia_il_tono() {
        let var: Vec<f64> = (0..=32).map(|n| n as f64).collect();
        let j = CohJudge { amp2: 4.0, nu: 1.0, piece: 8, max_pieces: 16, closed_penalty: 0.0, var_sum: &var };
        // 8 passi di tono d'ampiezza 2 sommati in fase contro 8 passi di solo rumore (modulo della somma circa la radice di 8)
        assert!(coherent_llr(&[(8, 16.0)], &j) > 5.0);
        assert!(coherent_llr(&[(8, 8f64.sqrt())], &j) < 0.0);
    }

    #[test]
    fn rumore_non_stampa() {
        let mut audio = vec![0.0f32; 12_000 * 15];
        noise(&mut audio, 0.05, 3);
        let mut rx = CwChain::new(700.0, 20.0);
        let n: usize = audio.chunks(1200).map(|b| rx.process(b).iter().filter(|c| **c != b' ').count()).sum();
        assert!(n <= 3, "{n}");
    }

    #[test]
    fn bigscan_apre_il_canale() {
        let mut audio = tone_audio(b"CQ TEST IU3ABC IU3ABC K", 1000.0, 25.0, 0.3);
        noise(&mut audio, 0.02, 5);
        let mut band = CwChainBand::new(150.0, 4_000.0, 20);
        for b in audio.chunks(1200) {
            band.process(b);
        }
        let ch = band.channels();
        assert!(ch.iter().any(|c| (c.freq_hz - 1000.0).abs() < 25.0 && c.text.contains("IU3ABC")), "{ch:?}");
    }
}
