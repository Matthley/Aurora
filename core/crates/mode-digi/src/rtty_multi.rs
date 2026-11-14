//! RTTY multicanale (passo 15 di `docs/progetto-rtty-oltre.md`): trova da solo i segnali RTTY nella banda audio e decodifica ognuno con
//! un ricevitore completo (`RttyDemodulator`, con le varianti, il modello del testo e lo squelch statistico).
//!
//! Un segnale RTTY e' una coppia di toni alla distanza dello spostamento che si alternano: un banco di filtri di Goertzel a passi di 5 Hz
//! misura la potenza di ogni frequenza su finestre lunghe un bit; ogni secondo, per ogni possibile centro, i due toni devono stare sopra
//! il fondo della banda (la mediana delle potenze) e le loro potenze bit per bit devono essere anticorrelate (quando c'e' il mark non c'e'
//! lo space). Una portante o un segnale largo non passano: una portante e' un tono solo, la voce e il rumore non alternano.
//! Un canale senza caratteri sicuri per 30 secondi si chiude.

use crate::psk::RATE;
use crate::rtty::{RttyChar, RttyDemodulator, RttyParams, SQUELCH_CLOSE, SQUELCH_OPEN};
use std::f64::consts::PI;

/// Passo del banco di filtri (Hz) e banda esplorata.
const BIN_HZ: f32 = 5.0;
// tutta la banda audio che si ascolta, come il CW di tutta la banda (150-4000 Hz): prima si fermava a 3000 Hz e una stazione piu' in alto
// non si vedeva
const LOW_HZ: f32 = 200.0;
const HIGH_HZ: f32 = 4000.0;
/// I due toni devono superare il fondo della banda di questo rapporto di potenza (circa 6 dB) e le loro potenze bit per bit essere
/// anticorrelate almeno cosi'.
const FLOOR_RATIO: f64 = 4.0;
const ANTI_CORR: f64 = -0.5;
/// Un canale nuovo non nasce a meno di questa distanza (in frazioni dello spostamento) da uno che c'e' gia'.
const SAME_FRACTION: f32 = 0.5;
/// Secondi senza caratteri sicuri dopo cui un canale si chiude, e canali al massimo.
const IDLE_S: f32 = 75.0;
/// Un carattere da questa probabilita' in su tiene aperto il canale (un segnale debole o in QSB legge spesso fra 0,7 e 0,9).
const ACTIVE_PROB: f32 = 0.7;
const MAX_CHANNELS: usize = 30;
/// La sintonia fine dei ricevitori dei canali (Hz): il centro nasce al passo del banco (2,5 Hz di errore) e la stazione puo' scivolare;
/// sul banco di prova BIGScan il ritardo mediano scende da 12 a 9 s.
const FINE_TUNING_HZ: f32 = 15.0;
/// Secondi d'audio gia' passato che un canale nuovo legge appena nasce: il segnale si riconosce dopo almeno un secondo e l'inizio del
/// CQ (spesso il nominativo) andava perso, cosi' la stazione si verificava solo al CQ seguente.
const PREROLL_S: f32 = 2.0;
/// Un canale il cui centro non si rivede fra i segnali per tanti secondi di fila, mentre accanto (entro `SAME_FRACTION`) se ne vede uno,
/// e' nato storto (una coppia sbagliata fra i toni di una stazione e quelli vicini): si sposta sul segnale vero. Prima due canali a
/// 970 e 1110 Hz tenevano occupata la stazione a 1035 Hz, che non si leggeva mai.
const MISPLACED_S: u32 = 3;
/// Entro questa distanza (Hz) un segnale trovato e' lo stesso del canale (passo del banco piu' scivolamento).
const OWN_HZ: f32 = 10.0;

/// Un segnale seguito.
struct Lane {
    centre_hz: f32,
    demod: RttyDemodulator,
    idle_s: f32,
    /// Appena nato: legge prima l'audio passato (`PREROLL_S`).
    fresh: bool,
    /// Secondi di fila in cui accanto si vede un segnale e al centro del canale no (`MISPLACED_S`).
    misplaced: u32,
}

/// I caratteri nuovi di un canale.
#[derive(Clone, Debug, PartialEq)]
pub struct LaneText {
    pub centre_hz: f32,
    pub chars: Vec<RttyChar>,
}

pub struct RttyMulti {
    shift_hz: f32,
    baud: f32,
    /// Lunghezza della finestra (un bit) e campioni gia' entrati in quella in corso.
    win: usize,
    fill: usize,
    /// Per ogni frequenza del banco: coefficiente di Goertzel e i due stati.
    coef: Vec<f64>,
    s1: Vec<f64>,
    s2: Vec<f64>,
    /// Le potenze delle ultime finestre (un secondo), finestra per finestra.
    history: Vec<Vec<f64>>,
    lanes: Vec<Lane>,
    /// L'audio degli ultimi `PREROLL_S` secondi, fino al blocco in corso escluso.
    past: std::collections::VecDeque<f32>,
}

impl RttyMulti {
    pub fn new(shift_hz: f32, baud: f32) -> Self {
        let bins = ((HIGH_HZ - LOW_HZ) / BIN_HZ) as usize + 1;
        let coef = (0..bins).map(|i| 2.0 * (2.0 * PI * f64::from(LOW_HZ + i as f32 * BIN_HZ) / f64::from(RATE)).cos()).collect();
        Self {
            shift_hz,
            baud,
            win: ((RATE / baud).round() as usize).max(8),
            fill: 0,
            coef,
            s1: vec![0.0; bins],
            s2: vec![0.0; bins],
            history: Vec::new(),
            lanes: Vec::new(),
            past: std::collections::VecDeque::new(),
        }
    }

    /// La radio spostata di poco: ogni segnale cambia centro di `audio_hz`; i ricevitori ripartono sul centro nuovo, quelli fuori da
    /// `lo_hz`-`hi_hz` (o dal banco) si chiudono. Le potenze misurate (della sintonia vecchia) si dimenticano.
    pub fn shift(&mut self, audio_hz: f32, lo_hz: f32, hi_hz: f32) {
        let (lo, hi) = (lo_hz.max(LOW_HZ), hi_hz.min(HIGH_HZ));
        self.lanes.retain(|l| (lo..=hi).contains(&(l.centre_hz + audio_hz)));
        for l in self.lanes.iter_mut() {
            l.centre_hz += audio_hz;
            l.demod = new_demod(l.centre_hz, self.shift_hz, self.baud);
        }
        self.history.clear();
        self.past.clear();
        self.s1.iter_mut().for_each(|v| *v = 0.0);
        self.s2.iter_mut().for_each(|v| *v = 0.0);
        self.fill = 0;
    }

    /// I centri dei canali aperti (Hz).
    pub fn channels(&self) -> Vec<f32> {
        self.lanes.iter().map(|l| l.centre_hz).collect()
    }

    /// Altro audio a 12 kHz; restituisce i caratteri nuovi di ogni canale che ne ha.
    pub fn process(&mut self, samples: &[f32]) -> Vec<LaneText> {
        for &s in samples {
            let x = f64::from(s);
            for i in 0..self.coef.len() {
                let s0 = x + self.coef[i] * self.s1[i] - self.s2[i];
                self.s2[i] = self.s1[i];
                self.s1[i] = s0;
            }
            self.fill += 1;
            if self.fill == self.win {
                self.fill = 0;
                let powers: Vec<f64> = (0..self.coef.len())
                    .map(|i| self.s1[i] * self.s1[i] + self.s2[i] * self.s2[i] - self.coef[i] * self.s1[i] * self.s2[i])
                    .collect();
                self.s1.iter_mut().for_each(|v| *v = 0.0);
                self.s2.iter_mut().for_each(|v| *v = 0.0);
                self.history.push(powers);
                if self.history.len() as f32 >= self.baud {
                    self.detect();
                    self.history.clear();
                }
            }
        }
        let mut out = Vec::new();
        let secs = samples.len() as f32 / RATE;
        let (a, b) = self.past.as_slices();
        for lane in &mut self.lanes {
            let mut chars = Vec::new();
            if lane.fresh {
                lane.fresh = false;
                chars.extend(lane.demod.process_chars(a));
                chars.extend(lane.demod.process_chars(b));
            }
            chars.extend(lane.demod.process_chars(samples));
            if chars.iter().any(|c| c.prob >= ACTIVE_PROB) {
                lane.idle_s = 0.0;
            } else {
                lane.idle_s += secs;
            }
            if !chars.is_empty() {
                out.push(LaneText { centre_hz: lane.centre_hz, chars });
            }
        }
        self.lanes.retain(|l| l.idle_s < IDLE_S);
        let keep = (PREROLL_S * RATE) as usize;
        self.past.extend(samples.iter().copied());
        let extra = self.past.len().saturating_sub(keep);
        self.past.drain(..extra);
        out
    }

    /// Un secondo di potenze: i centri dei segnali RTTY che non hanno ancora un canale.
    fn detect(&mut self) {
        let bins = self.coef.len();
        let n = self.history.len() as f64;
        let mean: Vec<f64> = (0..bins).map(|i| self.history.iter().map(|w| w[i]).sum::<f64>() / n).collect();
        let mut sorted = mean.clone();
        sorted.sort_by(f64::total_cmp);
        let floor = sorted[sorted.len() / 2].max(1e-12);
        let step = (self.shift_hz / BIN_HZ).round() as usize;
        if step == 0 || step >= bins {
            return;
        }
        // i due toni devono essere picchi (entro 10 Hz): i lobi laterali di un tono forte, a 50-80 Hz, non contano
        let peak = |k: usize| (k.saturating_sub(2)..=(k + 2).min(bins - 1)).all(|q| mean[q] <= mean[k]);
        // (centro, punteggio) dei candidati
        let mut found: Vec<(f32, f64)> = Vec::new();
        for i in 0..bins - step {
            let j = i + step;
            if mean[i] < FLOOR_RATIO * floor || mean[j] < FLOOR_RATIO * floor || !peak(i) || !peak(j) {
                continue;
            }
            let (mi, mj) = (mean[i], mean[j]);
            let (mut cov, mut vi, mut vj) = (0.0, 0.0, 0.0);
            for w in &self.history {
                let (a, b) = (w[i] - mi, w[j] - mj);
                cov += a * b;
                vi += a * a;
                vj += b * b;
            }
            let corr = cov / (vi * vj).sqrt().max(1e-30);
            if corr < ANTI_CORR {
                let centre = LOW_HZ + (i as f32 + step as f32 / 2.0) * BIN_HZ;
                found.push((centre, mi.min(mj) / floor * -corr));
            }
        }
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.recentre(&found);
        for &(centre, _) in &found {
            let near = |c: f32| (c - centre).abs() < SAME_FRACTION * self.shift_hz;
            if self.lanes.iter().any(|l| near(l.centre_hz)) || self.lanes.len() >= MAX_CHANNELS {
                continue;
            }
            self.lanes.push(Lane { centre_hz: centre, demod: new_demod(centre, self.shift_hz, self.baud), idle_s: 0.0, fresh: true, misplaced: 0 });
        }
    }

    /// I canali nati storti (vedi `MISPLACED_S`) si spostano sul segnale vero accanto; se li' c'e' gia' un canale si chiudono.
    fn recentre(&mut self, found: &[(f32, f64)]) {
        let tol = SAME_FRACTION * self.shift_hz;
        let mut moves: Vec<(usize, f32)> = Vec::new();
        for (k, lane) in self.lanes.iter_mut().enumerate() {
            let own = found.iter().any(|f| (f.0 - lane.centre_hz).abs() <= OWN_HZ);
            // il segnale accanto piu' forte (found e' in ordine di punteggio)
            let beside = found.iter().find(|f| (f.0 - lane.centre_hz).abs() > OWN_HZ && (f.0 - lane.centre_hz).abs() < tol);
            match (own, beside) {
                (false, Some(&(c, _))) => {
                    lane.misplaced += 1;
                    if lane.misplaced >= MISPLACED_S {
                        moves.push((k, c));
                    }
                }
                _ => lane.misplaced = 0,
            }
        }
        let mut drop = vec![false; self.lanes.len()];
        for (k, c) in moves {
            let taken = self.lanes.iter().enumerate().any(|(q, l)| q != k && !drop[q] && (l.centre_hz - c).abs() <= OWN_HZ);
            if taken {
                drop[k] = true;
                continue;
            }
            let l = &mut self.lanes[k];
            l.centre_hz = c;
            l.demod = new_demod(c, self.shift_hz, self.baud);
            l.fresh = true;
            l.misplaced = 0;
            l.idle_s = 0.0;
        }
        let mut k = 0;
        self.lanes.retain(|_| {
            k += 1;
            !drop[k - 1]
        });
    }
}

/// Il ricevitore di un canale: squelch statistico e sintonia fine entro `FINE_TUNING_HZ`.
fn new_demod(centre_hz: f32, shift_hz: f32, baud: f32) -> RttyDemodulator {
    let mut demod = RttyDemodulator::new(RttyParams { centre_hz, shift_hz, baud, reverse: false });
    demod.set_squelch_prob(Some((SQUELCH_OPEN, SQUELCH_CLOSE)));
    demod.set_fine_tuning(Some(FINE_TUNING_HZ));
    demod
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rtty::modulate;

    fn rumore(n: usize, ampiezza: f32, mut seed: u32) -> Vec<f32> {
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (seed as f32 / u32::MAX as f32 - 0.5) * ampiezza
            })
            .collect()
    }

    #[test]
    fn tre_stazioni_insieme_si_decodificano_tutte() {
        let testi = [(805.0, "CQ CQ DE IU3ABC IU3ABC K "), (1500.0, "QRZ DE IO91WM TEST TEST "), (2210.0, "TU 599 001 599 001 TU ")];
        let mut audio = vec![0.0_f32; 4 * 12_000];
        let mut parlato = Vec::new();
        for (f, t) in testi {
            let p = RttyParams { centre_hz: f, shift_hz: 170.0, baud: 45.45, reverse: false };
            let testo = t.repeat(4);
            let w = modulate(testo.as_bytes(), p, 0.2);
            if audio.len() < w.len() + 12_000 {
                audio.resize(w.len() + 12_000, 0.0);
            }
            for (a, s) in audio.iter_mut().zip(w.iter()) {
                *a += *s;
            }
            parlato.push((f, t));
        }
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 77)) {
            *a += r;
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        let mut testo: Vec<(f32, String)> = Vec::new();
        for blocco in audio.chunks(1200) {
            for l in multi.process(blocco) {
                let s: String = l.chars.iter().map(|c| c.ch).collect();
                match testo.iter_mut().find(|(c, _)| (*c - l.centre_hz).abs() < 1.0) {
                    Some((_, t)) => t.push_str(&s),
                    None => testo.push((l.centre_hz, s)),
                }
            }
        }
        for (f, t) in parlato {
            let parola = t.split(' ').find(|w| w.len() >= 6).unwrap_or(t);
            let ok = testo.iter().any(|(c, s)| (c - f).abs() < 30.0 && s.contains(parola));
            assert!(ok, "stazione a {f} Hz ({parola}) non trovata: canali {:?}", testo);
        }
        assert!(multi.channels().len() <= 4, "canali: {:?}", multi.channels());
    }

    /// Una stazione fuori di 14 Hz dal centro del canale (fra due passi del banco e scivolata), debole: con la sintonia fine si leggono 10
    /// nominativi su 12, senza 7.
    #[test]
    fn un_canale_fuori_centro_si_legge_con_la_sintonia_fine() {
        let p = RttyParams { centre_hz: 1014.0, shift_hz: 170.0, baud: 45.45, reverse: false };
        let mut audio = modulate("CQ TEST IU3ABC IU3ABC TEST ".repeat(6).as_bytes(), p, 0.2);
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 1.8, 11)) {
            *a += r;
        }
        let mut rx = new_demod(1000.0, 170.0, 45.45);
        let mut testo = String::new();
        for blocco in audio.chunks(1200) {
            testo.extend(rx.process_chars(blocco).iter().map(|c| c.ch));
        }
        assert!(testo.matches("IU3ABC").count() >= 9, "{} {testo}", testo.matches("IU3ABC").count());
    }

    /// Una chiamata breve dopo il silenzio: il canale nasce a trasmissione gia' iniziata, ma rilegge l'audio passato e il nominativo c'e'
    /// tutte e due le volte (senza, il primo andava perso).
    #[test]
    fn un_canale_nuovo_legge_anche_l_inizio_del_cq() {
        let p = RttyParams { centre_hz: 1500.0, shift_hz: 170.0, baud: 45.45, reverse: false };
        let mut audio = vec![0.0_f32; 10_800];
        audio.extend(modulate("IU3ABC IU3ABC TEST ".as_bytes(), p, 0.2));
        audio.extend(vec![0.0_f32; 2 * 12_000]);
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 3)) {
            *a += r;
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        let mut testo = String::new();
        for blocco in audio.chunks(1200) {
            for l in multi.process(blocco) {
                testo.extend(l.chars.iter().map(|c| c.ch));
            }
        }
        assert!(testo.matches("IU3ABC").count() >= 2, "{testo}");
    }

    /// Un canale nato storto 65 Hz sotto una stazione (e uno 75 Hz sopra) si sposta sulla stazione dopo qualche secondo e la legge;
    /// il secondo, ormai doppio, si chiude.
    #[test]
    fn un_canale_storto_si_sposta_sulla_stazione_vera() {
        let p = RttyParams { centre_hz: 1035.0, shift_hz: 170.0, baud: 45.45, reverse: false };
        let mut audio = modulate("CQ TEST IU3ABC IU3ABC TEST ".repeat(5).as_bytes(), p, 0.2);
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 9)) {
            *a += r;
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        for c in [970.0_f32, 1110.0] {
            multi.lanes.push(Lane { centre_hz: c, demod: new_demod(c, 170.0, 45.45), idle_s: 0.0, fresh: false, misplaced: 0 });
        }
        let mut testo = String::new();
        for blocco in audio.chunks(1200) {
            for l in multi.process(blocco) {
                if (l.centre_hz - 1035.0).abs() < 10.0 {
                    testo.extend(l.chars.iter().map(|c| c.ch));
                }
            }
        }
        let ch = multi.channels();
        assert!(ch.len() == 1 && (ch[0] - 1035.0).abs() <= 10.0, "canali: {ch:?}");
        assert!(testo.matches("IU3ABC").count() >= 3, "{testo}");
    }

    #[test]
    fn una_portante_e_il_rumore_non_aprono_canali() {
        let mut audio = rumore(5 * 12_000, 0.3, 5);
        for (i, a) in audio.iter_mut().enumerate() {
            *a += 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 12_000.0).sin();
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        multi.process(&audio);
        assert!(multi.channels().is_empty(), "canali: {:?}", multi.channels());
    }
}
