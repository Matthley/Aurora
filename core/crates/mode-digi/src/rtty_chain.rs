//! RTTY, ricevitore a catena probabilistica (alternativo a `rtty::RttyDemodulator`, stessa interfaccia `process_chars`):
//! - per ogni tono un rivelatore (oscillatore e somma mobile lunga un bit, letto otto volte per bit), con l'inviluppo (massimo su una
//!   finestra di qualche bit) e il fondo (minimo sulla stessa finestra);
//! - per ogni tono la verosimiglianza dell'ampiezza misurata se il tono e' acceso (livello all'inviluppo) o spento (al fondo), con un
//!   modello misto: una campana stretta intorno al livello, una larga e una coda esponenziale, che reggono scariche e fading; da qui la
//!   probabilita' che il tono sia acceso, p = L1 / (L0 + L1);
//! - i due toni si combinano: A = pM (1 - pS), B = pS (1 - pM), P(mark) = A / (A + B); un tono sparito (p vicino a 1/2) non trascina
//!   l'altro;
//! - la decodifica di sequenza di `rtty` (start, cinque bit, stop di durata variabile, riposo e silenzio) dice dove cominciano i caratteri;
//! - squelch a due livelli: l'errore grezzo del carattere, 1 - prodotto di max(p, 1 - p) sui cinque bit, apre e chiude secondo la storia
//!   degli ultimi caratteri; a squelch aperto un carattere poco sicuro aspetta il seguente.
//!
//! Tutti i valori sono tarati sul banco `tests/rtty_caratteri.rs`.

use crate::psk::RATE;
use crate::rtty::{RttyChar, RttyCharset, RttyParams, Seq, decode_char};
use std::collections::VecDeque;
use std::f64::consts::PI;

/// Passi per bit della catena.
const STEPS_PER_BIT: f64 = 8.0;
/// Finestre dell'inviluppo e del fondo (bit).
const ENV_BITS: f64 = 6.0;
const FLOOR_BITS: f64 = 6.0;
/// L'inviluppo non scende sotto questo multiplo del fondo.
const ENV_OVER_FLOOR: f64 = 1.1;
/// Il livello a tono spento non scende sotto il rumore medio accanto ai toni (il minimo della finestra e' troppo basso per il rumore).
const OFF_OVER_NOISE: f64 = 1.0;
/// Modello misto: larghezza della campana stretta in multipli del rumore medio, larghezza della larga in multipli della stretta, pesi della
/// larga e della coda.
const SIGMA_K: f64 = 0.6;
const WIDE_K: f64 = 3.0;
const W_WIDE: f64 = 0.03;
const W_TAIL: f64 = 0.03;
/// Dalle probabilita' combinate ai valori morbidi della sequenza, e dalle somme dei bit alla loro probabilita'.
const SEQ_GAIN: f32 = 0.35;
const BIT_GAIN: f32 = 0.35;
/// Squelch: errore grezzo oltre il quale il carattere e' debole, quanti caratteri ricorda, quanti deboli al massimo per aprire, quanti
/// deboli di fila per chiudere.
const RAW_WEAK: f32 = 0.2;
const SQ_MEMORY: usize = 4;
const SQ_OPEN_WEAK: usize = 1;
const SQ_CLOSE_RUN: usize = 3;
/// Media del rumore accanto ai toni (bit): misura il livello a tono spento, la larghezza delle campane e la presenza della portante.
const NOISE_BITS: f64 = 16.0;

const LTRS: u8 = 31;
const FIGS: u8 = 27;
const SPACE: u8 = 4;

struct Tone {
    freq: f64,
    ring: VecDeque<(f64, f64)>,
    sum: (f64, f64),
    phase: f64,
}

impl Tone {
    fn new(freq: f64) -> Self {
        Self { freq, ring: VecDeque::new(), sum: (0.0, 0.0), phase: 0.0 }
    }

    fn push(&mut self, x: f32, len: usize) {
        let z = (f64::from(x) * self.phase.cos(), -f64::from(x) * self.phase.sin());
        self.phase = (self.phase + 2.0 * PI * self.freq / f64::from(RATE)) % (2.0 * PI);
        self.sum.0 += z.0;
        self.sum.1 += z.1;
        self.ring.push_back(z);
        if self.ring.len() > len
            && let Some(old) = self.ring.pop_front()
        {
            self.sum.0 -= old.0;
            self.sum.1 -= old.1;
        }
    }

    fn amp(&self) -> f64 {
        (self.sum.0 * self.sum.0 + self.sum.1 * self.sum.1).sqrt()
    }
}

/// Massimo o minimo su una finestra scorrevole (coda monotona).
struct Extreme {
    len: usize,
    max: bool,
    q: VecDeque<(u64, f64)>,
    n: u64,
}

impl Extreme {
    fn new(len: usize, max: bool) -> Self {
        Self { len: len.max(1), max, q: VecDeque::new(), n: 0 }
    }

    fn push(&mut self, x: f64) -> f64 {
        while let Some(&(_, b)) = self.q.back() {
            if (self.max && b <= x) || (!self.max && b >= x) {
                self.q.pop_back();
            } else {
                break;
            }
        }
        self.q.push_back((self.n, x));
        while let Some(&(i, _)) = self.q.front() {
            if i + self.len as u64 <= self.n {
                self.q.pop_front();
            } else {
                break;
            }
        }
        self.n += 1;
        self.q.front().map_or(x, |v| v.1)
    }
}

/// La verosimiglianza dell'ampiezza `a` se il livello vero e' `x`, con il rumore medio `noise` accanto ai toni.
fn likelihood(a: f64, x: f64, noise: f64) -> f64 {
    let s = (SIGMA_K * noise).max(1e-12);
    let w = WIDE_K * s + 0.1 * x;
    let g = |m: f64, s: f64| (-(a - m).powi(2) / (2.0 * s * s)).exp() / s;
    let tau = (x + s).max(1e-12);
    (1.0 - W_WIDE - W_TAIL) * g(x, s) + W_WIDE * g(x, w) + W_TAIL * (-a / tau).exp() / tau
}

/// La probabilita' che un tono sia acceso.
fn tone_prob(a: f64, env: f64, off: f64, noise: f64) -> f64 {
    let l1 = likelihood(a, env, noise);
    let l0 = likelihood(a, off, noise);
    if l0 + l1 > 0.0 { l1 / (l0 + l1) } else { 0.5 }
}

/// La probabilita' di mark dai due toni.
fn combine(pm: f64, ps: f64) -> f64 {
    let a = pm * (1.0 - ps);
    let b = ps * (1.0 - pm);
    if a + b > 0.0 { a / (a + b) } else { 0.5 }
}

struct Char {
    code: u8,
    probs: [f32; 7],
}

impl Char {
    fn raw_error(&self) -> f32 {
        1.0 - self.probs[1..6].iter().map(|p| p.max(1.0 - p)).product::<f32>()
    }

    fn confidence(&self) -> f32 {
        (1.0 - self.probs[0]) * self.probs[6] * self.probs[1..6].iter().map(|p| p.max(1.0 - p)).product::<f32>()
    }
}

pub struct RttyChain {
    params: RttyParams,
    tones: [Tone; 2],
    bit_len: usize,
    hop: usize,
    fill: usize,
    env: [Extreme; 2],
    floor: [Extreme; 2],
    noise: f64,
    seq: Seq,
    /// Valori morbidi per passo (anello) e passi gia' entrati.
    llr: Vec<f32>,
    steps: u64,
    pending: VecDeque<f64>,
    figures: bool,
    squelch: bool,
    hist: VecDeque<bool>,
    held: Vec<Char>,
}

impl RttyChain {
    pub fn new(params: RttyParams) -> Self {
        let spb = f64::from(RATE / params.baud);
        let hop = ((spb / STEPS_PER_BIT).round() as usize).max(1);
        let steps_bit = spb / hop as f64;
        let (mark, space) = if params.reverse { (params.space_hz(), params.mark_hz()) } else { (params.mark_hz(), params.space_hz()) };
        Self {
            params,
            tones: [Tone::new(f64::from(mark)), Tone::new(f64::from(space))],
            bit_len: spb.round() as usize,
            hop,
            fill: 0,
            env: [Extreme::new((ENV_BITS * steps_bit) as usize, true), Extreme::new((ENV_BITS * steps_bit) as usize, true)],
            floor: [Extreme::new((FLOOR_BITS * steps_bit) as usize, false), Extreme::new((FLOOR_BITS * steps_bit) as usize, false)],
            noise: 0.0,
            seq: Seq::new(steps_bit),
            llr: vec![0.0; 2048],
            steps: 0,
            pending: VecDeque::new(),
            figures: false,
            squelch: false,
            hist: VecDeque::new(),
            held: Vec::new(),
        }
    }

    pub fn params(&self) -> RttyParams {
        self.params
    }

    fn sum(&self, c: f64) -> f32 {
        let c = c.round() as i64;
        let n = self.llr.len() as i64;
        (c - 3..=c + 3).filter(|&i| i >= 0 && (self.steps as i64 - i) < n && i < self.steps as i64).map(|i| self.llr[(i % n) as usize]).sum()
    }

    pub fn process_chars(&mut self, samples: &[f32]) -> Vec<RttyChar> {
        let mut out = Vec::new();
        for &x in samples {
            let len = self.bit_len;
            for t in &mut self.tones {
                t.push(x, len);
            }
            self.fill += 1;
            if self.fill < self.hop {
                continue;
            }
            self.fill = 0;
            let a = [self.tones[0].amp(), self.tones[1].amp()];
            let lo = a[0].min(a[1]);
            let w = NOISE_BITS * STEPS_PER_BIT;
            self.noise = if self.noise == 0.0 { lo } else { self.noise + (lo.min(3.0 * self.noise) - self.noise) / w };
            let mut p = [0.5; 2];
            for k in 0..2 {
                // livello a tono spento: il minimo della finestra, non sotto il rumore medio accanto; a tono acceso: il massimo
                let off = self.floor[k].push(a[k]).max(OFF_OVER_NOISE * self.noise);
                let env = self.env[k].push(a[k]).max(ENV_OVER_FLOOR * off);
                p[k] = tone_prob(a[k], env, off, self.noise);
            }
            let pm = combine(p[0], p[1]).clamp(1e-6, 1.0 - 1e-6);
            let l = (pm / (1.0 - pm)).ln() as f32;
            let inst = (a[0].max(a[1]) / self.noise.max(1e-12)) as f32;
            let n = self.llr.len() as u64;
            self.llr[(self.steps % n) as usize] = l;
            self.steps += 1;
            let starts = self.seq.push(SEQ_GAIN * l / crate::rtty::LLR_SCALE, inst);
            self.pending.extend(starts.into_iter().map(|s| s + STEPS_PER_BIT / 2.0));
            while let Some(&c) = self.pending.front() {
                if c + 6.0 * STEPS_PER_BIT + 4.0 > self.steps as f64 {
                    break;
                }
                self.pending.pop_front();
                let mut probs = [0.0f32; 7];
                let mut code = 0u8;
                for (k, pr) in probs.iter_mut().enumerate() {
                    let s = self.sum(c + k as f64 * STEPS_PER_BIT);
                    *pr = 1.0 / (1.0 + (-BIT_GAIN * s).exp());
                    if (1..6).contains(&k) && s > 0.0 {
                        code |= 1 << (k - 1);
                    }
                }
                self.gate(Char { code, probs }, &mut out);
            }
        }
        out
    }

    /// Lo squelch a due livelli.
    fn gate(&mut self, ch: Char, out: &mut Vec<RttyChar>) {
        let weak = ch.raw_error() > RAW_WEAK;
        if self.hist.len() == SQ_MEMORY {
            self.hist.pop_front();
        }
        self.hist.push_back(weak);
        self.held.push(ch);
        if !self.squelch {
            if self.held.len() > SQ_MEMORY {
                self.held.remove(0);
            }
            if self.hist.len() == SQ_MEMORY && self.hist.iter().filter(|w| **w).count() <= SQ_OPEN_WEAK {
                self.squelch = true;
                let first = self.held.iter().position(|c| c.raw_error() <= RAW_WEAK).unwrap_or(0);
                let ready: Vec<Char> = self.held.drain(..).skip(first).collect();
                for c in ready {
                    self.emit(c, out);
                }
            }
            return;
        }
        let run = self.hist.iter().rev().take_while(|w| **w).count();
        if run >= SQ_CLOSE_RUN {
            self.squelch = false;
            self.held.clear();
            return;
        }
        if !weak || self.held.len() >= SQ_CLOSE_RUN {
            let ready = std::mem::take(&mut self.held);
            for c in ready {
                self.emit(c, out);
            }
        }
    }

    fn emit(&mut self, c: Char, out: &mut Vec<RttyChar>) {
        match c.code {
            LTRS => self.figures = false,
            FIGS => self.figures = true,
            0 => {}
            code => {
                if code == SPACE {
                    self.figures = false;
                }
                let ch = decode_char(RttyCharset::Ita2, code, self.figures);
                out.push(RttyChar { ch, prob: c.confidence() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtty::modulate;

    #[test]
    fn la_catena_legge_un_testo_pulito() {
        let p = RttyParams::default();
        let mut a = vec![0.0f32; 6000];
        a.extend(modulate(b"\r\nCQ TEST IU3ABC IU3ABC 599 001\r\n", p, 0.5));
        a.extend(vec![0.0f32; 12_000]);
        let mut rx = RttyChain::new(p);
        let s: String = rx.process_chars(&a).iter().map(|c| c.ch).collect();
        assert!(s.contains("CQ TEST IU3ABC IU3ABC 599 001"), "{s:?}");
    }

    #[test]
    fn i_due_toni_si_combinano() {
        assert!((combine(0.5, 0.5) - 0.5).abs() < 1e-9);
        // un tono che non dice nulla lascia decidere l'altro
        assert!(combine(0.9, 0.5) > 0.85);
        assert!(combine(0.5, 0.9) < 0.15);
    }
}
