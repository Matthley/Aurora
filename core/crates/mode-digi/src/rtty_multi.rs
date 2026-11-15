//! RTTY multicanale (passo 15 di `docs/progetto-rtty-oltre.md`): trova da solo i segnali RTTY nella banda audio e decodifica ognuno con
//! un ricevitore completo (`RttyDemodulator`, con le varianti, il modello del testo e lo squelch statistico).
//!
//! Un segnale RTTY e' una coppia di toni alla distanza dello spostamento che si alternano: un banco di filtri di Goertzel a passi di 5 Hz
//! misura la potenza di ogni frequenza su finestre lunghe un bit; ogni mezzo secondo, sull'ultimo secondo e per ogni possibile centro, i due toni devono stare sopra
//! il fondo della banda (la mediana delle potenze) e le loro potenze bit per bit devono essere anticorrelate (quando c'e' il mark non c'e'
//! lo space). Una portante o un segnale largo non passano: una portante e' un tono solo, la voce e il rumore non alternano.
//! Spostamento (170/425/850 Hz) e velocita' (45,45/50/75 baud) si riconoscono per ogni canale: quelli impostati valgono per primi.
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
/// Analisi al secondo (la finestra scorre di mezzo secondo).
const DETECTS_PER_S: u32 = 2;
/// Entro questa distanza (Hz) un segnale trovato e' lo stesso del canale (passo del banco piu' scivolamento).
const OWN_HZ: f32 = 10.0;
/// Gli spostamenti e le velocita' che si riconoscono canale per canale (oltre a quelli impostati): una stazione a 425 o 850 Hz, o a 50 o
/// 75 baud, si legge anche se la banda e' impostata a 170 Hz e 45,45 baud.
const SHIFTS: [f32; 3] = [170.0, 425.0, 850.0];
const BAUDS: [f32; 3] = [45.45, 50.0, 75.0];
/// La velocita' di un canale nuovo (`pick_baud`): un'altra vince su quella impostata solo se i cambi fra mark e space cadono sul suo passo
/// almeno cosi' bene, e meglio di tanto. Sul segnale giusto il punteggio e' circa 0,99, sulle velocita' sbagliate fra -0,3 e 0,15.
const LATTICE_MIN: f64 = 0.6;
const LATTICE_MARGIN: f64 = 0.3;

/// Un segnale seguito.
struct Lane {
    centre_hz: f32,
    /// Spostamento e velocita' riconosciuti per questo segnale.
    shift_hz: f32,
    baud: f32,
    demod: RttyDemodulator,
    idle_s: f32,
    /// Appena nato: legge prima l'audio passato (`PREROLL_S`).
    fresh: bool,
    /// Analisi di fila in cui accanto si vede un segnale e al centro del canale no (`MISPLACED_S`).
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
    /// Il massimo dei ricevitori attivi (lo abbassa il controllo del carico della banda).
    max_lanes: usize,
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
            max_lanes: MAX_CHANNELS,
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
            l.demod = new_demod(l.centre_hz, l.shift_hz, l.baud);
        }
        self.history.clear();
        self.past.clear();
        self.s1.iter_mut().for_each(|v| *v = 0.0);
        self.s2.iter_mut().for_each(|v| *v = 0.0);
        self.fill = 0;
    }

    /// I centri dei canali aperti (Hz).
    /// Limita i ricevitori attivi a `n` (1..=`MAX_CHANNELS`): quelli in piu', gli ultimi aperti, si chiudono subito.
    pub fn set_max_lanes(&mut self, n: usize) {
        self.max_lanes = n.clamp(1, MAX_CHANNELS);
        self.lanes.truncate(self.max_lanes);
    }

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
                // finestra di un secondo che scorre di mezzo secondo
                if self.history.len() as f32 >= self.baud {
                    self.detect();
                    let half = self.history.len() / 2;
                    self.history.drain(..half);
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

    /// Un secondo di potenze: i centri dei segnali RTTY che non hanno ancora un canale, per ogni spostamento di `SHIFTS` (quello
    /// impostato per primo); la velocita' di ogni canale nuovo la sceglie `pick_baud`.
    fn detect(&mut self) {
        let bins = self.coef.len();
        let n = self.history.len() as f64;
        let mean: Vec<f64> = (0..bins).map(|i| self.history.iter().map(|w| w[i]).sum::<f64>() / n).collect();
        let mut sorted = mean.clone();
        sorted.sort_by(f64::total_cmp);
        let floor = sorted[sorted.len() / 2].max(1e-12);
        // i due toni devono essere picchi (entro 10 Hz): i lobi laterali di un tono forte, a 50-80 Hz, non contano
        let peak = |k: usize| (k.saturating_sub(2)..=(k + 2).min(bins - 1)).all(|q| mean[q] <= mean[k]);
        let mut shifts = vec![self.shift_hz];
        shifts.extend(SHIFTS.iter().copied().filter(|s| (s - self.shift_hz).abs() > 1.0));
        // (centro, punteggio, spostamento) dei candidati
        let mut found: Vec<(f32, f64, f32)> = Vec::new();
        for (rank, &shift) in shifts.iter().enumerate() {
            let step = (shift / BIN_HZ).round() as usize;
            if step == 0 || step >= bins {
                continue;
            }
            let mut these: Vec<(f32, f64, f32)> = Vec::new();
            for i in 0..bins - step - 1 {
                // per gli spostamenti larghi i toni cadono spesso fra due passi del banco e i picchi possono stare a un passo in piu' o in
                // meno; per quello impostato no (sul banco di prova si aprivano canali falsi)
                let steps: &[usize] = if rank == 0 { &[step] } else { &[step, step - 1, step + 1] };
                let Some(j) = steps.iter().map(|d| i + d).find(|&j| peak(j)) else { continue };
                if mean[i] < FLOOR_RATIO * floor || mean[j] < FLOOR_RATIO * floor || !peak(i) {
                    continue;
                }
                // uno spostamento diverso da quello impostato non vale se uno dei due toni e' gia' di un segnale trovato
                let tone = |hz: f32| found.iter().any(|f| (hz - (f.0 - f.2 / 2.0)).abs() <= OWN_HZ || (hz - (f.0 + f.2 / 2.0)).abs() <= OWN_HZ);
                let (fi, fj) = (LOW_HZ + i as f32 * BIN_HZ, LOW_HZ + j as f32 * BIN_HZ);
                if rank > 0 && (tone(fi) || tone(fj)) {
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
                    let centre = LOW_HZ + (i + j) as f32 / 2.0 * BIN_HZ;
                    these.push((centre, mi.min(mj) / floor * -corr, shift));
                }
            }
            found.extend(these);
        }
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        self.recentre(&found);
        let mut past: Option<Vec<f32>> = None;
        for &(centre, _, shift) in &found {
            let near = |l: &Lane| (l.centre_hz - centre).abs() < SAME_FRACTION * l.shift_hz.max(shift);
            if self.lanes.iter().any(near) || self.lanes.len() >= self.max_lanes {
                continue;
            }
            let audio = past.get_or_insert_with(|| self.past.iter().copied().collect());
            let baud = pick_baud(audio, centre, shift, self.baud);
            self.lanes.push(Lane { centre_hz: centre, shift_hz: shift, baud, demod: new_demod(centre, shift, baud), idle_s: 0.0, fresh: true, misplaced: 0 });
        }
    }

    /// I canali nati storti (vedi `MISPLACED_S`) si spostano sul segnale vero accanto; se li' c'e' gia' un canale si chiudono.
    fn recentre(&mut self, found: &[(f32, f64, f32)]) {
        let mut moves: Vec<(usize, f32, f32)> = Vec::new();
        for (k, lane) in self.lanes.iter_mut().enumerate() {
            let tol = SAME_FRACTION * lane.shift_hz;
            let own = found.iter().any(|f| (f.0 - lane.centre_hz).abs() <= OWN_HZ);
            // il segnale accanto piu' forte (found e' in ordine di punteggio)
            let beside = found.iter().find(|f| (f.0 - lane.centre_hz).abs() > OWN_HZ && (f.0 - lane.centre_hz).abs() < tol);
            match (own, beside) {
                (false, Some(&(c, _, shift))) => {
                    lane.misplaced += 1;
                    if lane.misplaced >= MISPLACED_S * DETECTS_PER_S {
                        moves.push((k, c, shift));
                    }
                }
                _ => lane.misplaced = 0,
            }
        }
        let mut drop = vec![false; self.lanes.len()];
        for (k, c, shift) in moves {
            let taken = self.lanes.iter().enumerate().any(|(q, l)| q != k && !drop[q] && (l.centre_hz - c).abs() <= OWN_HZ);
            if taken {
                drop[k] = true;
                continue;
            }
            let l = &mut self.lanes[k];
            l.centre_hz = c;
            l.shift_hz = shift;
            l.demod = new_demod(c, shift, l.baud);
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

/// La velocita' di un segnale nuovo a `centre_hz`: fra `BAUDS` quella su cui i cambi fra mark e space dell'audio `past` cadono a passo
/// di mezzo bit (`lattice`); `default` (la velocita' impostata) se nessun'altra lo fa nettamente meglio. Prima vinceva quella che leggeva
/// piu' caratteri sicuri: sul secondo scarso di segnale di un canale appena nato il ricevitore giusto, ancora con lo squelch chiuso, ne
/// leggeva 0-2 e quello a 75 baud ne inventava 3-5 (ne legge di piu' perche' sono piu' corti), e un QTC a 45,45 baud usciva spazzatura.
fn pick_baud(past: &[f32], centre_hz: f32, shift_hz: f32, default: f32) -> f32 {
    let base = lattice(past, centre_hz, shift_hz, default);
    let mut best = (default, base + LATTICE_MARGIN);
    for &b in BAUDS.iter().filter(|b| (**b - default).abs() > 0.1) {
        let score = lattice(past, centre_hz, shift_hz, b);
        if score >= LATTICE_MIN && score > best.1 {
            best = (b, score);
        }
    }
    best.0
}

/// Quanto i cambi fra mark e space dell'audio `past` cadono su multipli di mezzo bit a `baud` (da -1 a 1; mezzo bit perche' lo stop
/// dura 1,5 bit). Non dipende da quale tono e' il mark.
fn lattice(past: &[f32], centre_hz: f32, shift_hz: f32, baud: f32) -> f64 {
    let len = ((RATE / shift_hz).round() as usize).max(4);
    let tones = [f64::from(centre_hz - shift_hz / 2.0), f64::from(centre_hz + shift_hz / 2.0)];
    let mut prod: Vec<[(f64, f64); 2]> = Vec::with_capacity(past.len());
    let mut sum = [(0.0_f64, 0.0_f64); 2];
    let mut d = Vec::with_capacity(past.len());
    for (n, &x) in past.iter().enumerate() {
        let mut p = [(0.0, 0.0); 2];
        for (k, f) in tones.iter().enumerate() {
            let w = 2.0 * PI * f * n as f64 / f64::from(RATE);
            p[k] = (f64::from(x) * w.cos(), -f64::from(x) * w.sin());
            sum[k].0 += p[k].0;
            sum[k].1 += p[k].1;
            if n >= len {
                sum[k].0 -= prod[n - len][k].0;
                sum[k].1 -= prod[n - len][k].1;
            }
        }
        prod.push(p);
        d.push((sum[1].0 * sum[1].0 + sum[1].1 * sum[1].1) - (sum[0].0 * sum[0].0 + sum[0].1 * sum[0].1));
    }
    if d.is_empty() {
        return 0.0;
    }
    let h = 0.2 * d.iter().map(|v| v.abs()).sum::<f64>() / d.len() as f64;
    let bit = f64::from(RATE / baud);
    let (lo, hi) = (f64::from(RATE / BAUDS[2]) / 2.0, 8.0 * f64::from(RATE / BAUDS[0]));
    let (mut state, mut last, mut acc, mut cnt) = (0_i8, None::<usize>, 0.0, 0_usize);
    for (n, &v) in d.iter().enumerate() {
        let now = if v > h { 1 } else if v < -h { -1 } else { state };
        if now != state && state != 0 {
            if let Some(l) = last {
                let gap = (n - l) as f64;
                if (lo..=hi).contains(&gap) {
                    acc += (4.0 * PI * gap / bit).cos();
                    cnt += 1;
                }
            }
            last = Some(n);
        }
        state = now;
    }
    if cnt == 0 { 0.0 } else { acc / cnt as f64 }
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

    #[test]
    fn il_limite_del_carico_ferma_i_ricevitori_in_piu() {
        let mut audio = vec![0.0_f32; 3 * 12_000];
        for f in [805.0, 1500.0, 2210.0] {
            let p = RttyParams { centre_hz: f, shift_hz: 170.0, baud: 45.45, reverse: false };
            let w = modulate("CQ CQ DE IU3ABC IU3ABC K ".repeat(3).as_bytes(), p, 0.2);
            if audio.len() < w.len() {
                audio.resize(w.len(), 0.0);
            }
            for (a, s) in audio.iter_mut().zip(w.iter()) {
                *a += *s;
            }
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        let meta = audio.len() / 2;
        for blocco in audio[..meta].chunks(1200) {
            multi.process(blocco);
        }
        assert!(multi.channels().len() >= 2, "canali: {:?}", multi.channels());
        multi.set_max_lanes(1);
        assert_eq!(multi.channels().len(), 1);
        for blocco in audio[meta..].chunks(1200) {
            multi.process(blocco);
            assert!(multi.channels().len() <= 1, "canali: {:?}", multi.channels());
        }
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
            multi.lanes.push(Lane { centre_hz: c, shift_hz: 170.0, baud: 45.45, demod: new_demod(c, 170.0, 45.45), idle_s: 0.0, fresh: false, misplaced: 0 });
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
    /// La finestra che scorre di mezzo secondo: una stazione che parte a 0,6 s ha il canale entro 1,6 s (a blocchi fissi di un secondo
    /// solo a 2 s).
    #[test]
    fn il_canale_si_apre_entro_mezzo_secondo_dalla_finestra_piena() {
        let p = RttyParams { centre_hz: 1500.0, shift_hz: 170.0, baud: 45.45, reverse: false };
        let mut audio = vec![0.0_f32; 7_200];
        audio.extend(modulate("RYRYRYRY CQ IU3ABC IU3ABC ".as_bytes(), p, 0.3));
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 19)) {
            *a += r;
        }
        let mut multi = RttyMulti::new(170.0, 45.45);
        let mut aperto = None;
        for (k, blocco) in audio.chunks(600).enumerate() {
            multi.process(blocco);
            if aperto.is_none() && !multi.channels().is_empty() {
                aperto = Some((k + 1) as f32 * 0.05);
            }
        }
        assert!(aperto.is_some_and(|t| t <= 1.65), "aperto a {aperto:?} s");
    }
    fn leggi(multi: &mut RttyMulti, p: RttyParams, testo: &str) -> String {
        let mut audio = vec![0.0_f32; 6_000];
        audio.extend(modulate(testo.as_bytes(), p, 0.3));
        let n = audio.len();
        for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 23)) {
            *a += r;
        }
        let mut letto = String::new();
        for blocco in audio.chunks(1200) {
            for l in multi.process(blocco) {
                letto.extend(l.chars.iter().map(|c| c.ch));
            }
        }
        letto
    }

    /// Banda impostata a 170 Hz: una stazione a 425 Hz di spostamento ha il suo canale a 425 Hz e si legge.
    #[test]
    fn lo_spostamento_si_riconosce_per_canale() {
        let p = RttyParams { centre_hz: 1500.0, shift_hz: 425.0, baud: 45.45, reverse: false };
        let mut multi = RttyMulti::new(170.0, 45.45);
        let letto = leggi(&mut multi, p, &"CQ TEST IU3ABC IU3ABC TEST ".repeat(4));
        assert_eq!(multi.lanes.len(), 1, "canali: {:?}", multi.channels());
        assert!((multi.lanes[0].centre_hz - 1500.0).abs() <= 10.0 && multi.lanes[0].shift_hz == 425.0, "{} {}", multi.lanes[0].centre_hz, multi.lanes[0].shift_hz);
        assert!(letto.contains("IU3ABC"), "{letto}");
    }

    /// Banda impostata a 45,45 baud: una stazione a 75 baud ha il suo canale a 75 baud e si legge.
    #[test]
    fn la_velocita_si_riconosce_per_canale() {
        let p = RttyParams { centre_hz: 1500.0, shift_hz: 170.0, baud: 75.0, reverse: false };
        let mut multi = RttyMulti::new(170.0, 45.45);
        let letto = leggi(&mut multi, p, &"CQ TEST IU3ABC IU3ABC TEST ".repeat(6));
        assert_eq!(multi.lanes.len(), 1, "canali: {:?}", multi.channels());
        assert_eq!(multi.lanes[0].baud, 75.0);
        assert!(letto.contains("IU3ABC"), "{letto}");
    }

    const QTC: &str = "QTC 22/10 QRV? / 1655-DK1WI-311 / 1656-S50A-123 / 1657-DK8EY-111 / 1658-DG2BWG-259 / 1700-DL5NO-2 ";

    /// Un QTC da contest a 45,45 baud (cifre, trattini, cambi FIGS/LTRS), solo o con una stazione accanto: il canale nasce a 45,45 baud
    /// e si legge. Prima nasceva a 75 baud e la cascata mostrava «VVMTVMMLVOTVV...».
    #[test]
    fn un_qtc_a_45_baud_non_nasce_a_75() {
        let p = RttyParams { centre_hz: 1500.0, shift_hz: 170.0, baud: 45.45, reverse: false };
        for vicina in [false, true] {
            let mut audio = vec![0.0_f32; 6_000];
            audio.extend(modulate(QTC.repeat(3).as_bytes(), p, 0.3));
            if vicina {
                let q = RttyParams { centre_hz: 1250.0, ..p };
                let v = modulate("CQ TEST IU3ABC IU3ABC TEST ".repeat(8).as_bytes(), q, 0.3);
                for (a, x) in audio.iter_mut().zip(v) {
                    *a += x;
                }
            }
            let n = audio.len();
            for (a, r) in audio.iter_mut().zip(rumore(n, 0.3, 23)) {
                *a += r;
            }
            let mut multi = RttyMulti::new(170.0, 45.45);
            let mut letto = String::new();
            for blocco in audio.chunks(1200) {
                for l in multi.process(blocco) {
                    if (l.centre_hz - 1500.0).abs() < 50.0 {
                        letto.extend(l.chars.iter().map(|c| c.ch));
                    }
                }
            }
            let canale = multi.lanes.iter().find(|l| (l.centre_hz - 1500.0).abs() < 50.0);
            assert!(canale.is_some_and(|l| l.baud == 45.45), "vicina {vicina}: {:?}", multi.lanes.iter().map(|l| (l.centre_hz, l.baud)).collect::<Vec<_>>());
            assert!(letto.contains("1657-DK8EY-111"), "vicina {vicina}: {letto}");
        }
    }
}

