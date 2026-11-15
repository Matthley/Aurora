//! Ricevitore CW «veloce»: la catena ([`crate::cw_chain::CwChain`]) mostra subito un testo provvisorio, il ricevitore attuale
//! ([`crate::cw::CwDemodulator`]) decide il testo definitivo. Quando arrivano caratteri definitivi si allineano (distanza di Levenshtein,
//! semi-globale) all'inizio del testo provvisorio e ne tolgono la parte che coprono; i caratteri provvisori uguali a quelli definitivi
//! valgono come mostrati al momento del provvisorio. La correttezza finale e' quella del ricevitore attuale per costruzione.

use std::collections::VecDeque;

use crate::RATE;
use crate::cw::CwDemodulator;
use crate::cw_chain::{ChainParams, CwChain};

/// Eta' oltre la quale un carattere provvisorio mai confermato si toglie (secondi).
const MAX_AGE_S: f64 = 3.0;
/// Spostamento del tono oltre il quale la catena riparte sulla nuova frequenza (Hz).
const RETUNE_HZ: f32 = 5.0;

/// Un carattere definitivo con il momento (secondi d'audio) in cui e' comparso giusto sullo schermo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shown {
    pub ch: u8,
    pub at_s: f64,
}

/// Contatori del testo provvisorio.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HybridStats {
    /// Caratteri provvisori mostrati.
    pub provisional: u64,
    /// Provvisori confermati uguali dal definitivo.
    pub confirmed: u64,
}

/// Il risultato di un blocco d'audio.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HybridOut {
    /// Caratteri definitivi nuovi.
    pub fixed: Vec<Shown>,
    /// Testo provvisorio intero dopo il blocco (sostituisce il precedente), se e' cambiato.
    pub provisional: Option<String>,
}

pub struct CwHybrid {
    fine: CwDemodulator,
    fast: CwChain,
    params: ChainParams,
    tone: f32,
    wpm_hint: f32,
    pending: VecDeque<(u8, f64)>,
    now_s: f64,
    stats: HybridStats,
}

impl CwHybrid {
    /// Il ricevitore attuale gia' impostato e la catena sullo stesso tono.
    pub fn new(fine: CwDemodulator, wpm_hint: f32) -> Self {
        Self::with_params(fine, wpm_hint, &ChainParams::default())
    }

    pub fn with_params(fine: CwDemodulator, wpm_hint: f32, pr: &ChainParams) -> Self {
        let tone = fine.frequency();
        Self {
            fast: CwChain::with_params(tone, wpm_hint, pr),
            fine,
            params: pr.clone(),
            tone,
            wpm_hint,
            pending: VecDeque::new(),
            now_s: 0.0,
            stats: HybridStats::default(),
        }
    }

    pub fn fine(&self) -> &CwDemodulator {
        &self.fine
    }

    pub fn fine_mut(&mut self) -> &mut CwDemodulator {
        &mut self.fine
    }

    pub fn stats(&self) -> HybridStats {
        self.stats
    }

    /// Il testo provvisorio adesso.
    pub fn provisional(&self) -> String {
        self.pending.iter().map(|p| char::from(p.0)).collect()
    }

    /// Porta l'ascolto su `hz`; la catena riparte se il tono si e' spostato.
    pub fn steer(&mut self, hz: f32) {
        self.fine.steer(hz);
        self.follow_tone();
    }

    fn follow_tone(&mut self) {
        let f = self.fine.frequency();
        if (f - self.tone).abs() > RETUNE_HZ {
            self.tone = f;
            self.fast = CwChain::with_params(f, self.fine.wpm().max(self.wpm_hint.min(60.0)), &self.params);
        }
    }

    pub fn process(&mut self, audio: &[f32]) -> HybridOut {
        self.now_s += audio.len() as f64 / f64::from(RATE);
        let now = self.now_s;
        let before = self.pending.len();
        let mut changed = false;
        let quick = self.fast.process(audio);
        changed |= !quick.is_empty();
        self.stats.provisional += quick.iter().filter(|c| **c != b' ').count() as u64;
        self.pending.extend(quick.into_iter().map(|c| (c, now)));
        let fixed = self.fine.process(audio);
        self.follow_tone();
        let mut shown = Vec::with_capacity(fixed.len());
        if !fixed.is_empty() {
            changed = true;
            self.settle(&fixed, now, &mut shown);
        }
        while self.pending.front().is_some_and(|p| now - p.1 > MAX_AGE_S) {
            self.pending.pop_front();
            changed = true;
        }
        // spazi in testa al provvisorio non dicono nulla
        while self.pending.front().is_some_and(|p| p.0 == b' ') {
            self.pending.pop_front();
            changed |= before > 0;
        }
        HybridOut { fixed: shown, provisional: changed.then(|| self.provisional()) }
    }

    /// Allinea i caratteri definitivi `f` all'inizio del provvisorio e toglie la parte coperta.
    fn settle(&mut self, f: &[u8], now: f64, shown: &mut Vec<Shown>) {
        let p: Vec<(u8, f64)> = self.pending.iter().copied().collect();
        let (m, n) = (f.len(), p.len());
        // d[i][j]: costo di f[..i] contro p[..j]
        let w = n + 1;
        let mut d = vec![0u32; (m + 1) * w];
        for j in 0..=n {
            d[j] = j as u32;
        }
        for i in 1..=m {
            d[i * w] = i as u32;
            for j in 1..=n {
                let sub = d[(i - 1) * w + j - 1] + u32::from(f[i - 1] != p[j - 1].0);
                d[i * w + j] = sub.min(d[(i - 1) * w + j] + 1).min(d[i * w + j - 1] + 1);
            }
        }
        // la parte del provvisorio coperta: costo minimo, a parita' la piu' lunga
        let mut best = 0;
        for j in 0..=n {
            if d[m * w + j] <= d[m * w + best] {
                best = j;
            }
        }
        // risalita: i caratteri uguali prendono il momento del provvisorio
        let mut at = vec![now; m];
        let (mut i, mut j) = (m, best);
        while i > 0 && j > 0 {
            let here = d[i * w + j];
            let same = f[i - 1] == p[j - 1].0;
            if here == d[(i - 1) * w + j - 1] + u32::from(!same) {
                if same {
                    at[i - 1] = p[j - 1].1;
                    if f[i - 1] != b' ' {
                        self.stats.confirmed += 1;
                    }
                }
                i -= 1;
                j -= 1;
            } else if here == d[(i - 1) * w + j] + 1 {
                i -= 1;
            } else {
                j -= 1;
            }
        }
        self.pending.drain(..best);
        shown.extend(f.iter().zip(at).map(|(&ch, at_s)| Shown { ch, at_s }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hybrid() -> CwHybrid {
        CwHybrid::new(CwDemodulator::new(700.0, 20.0), 20.0)
    }

    #[test]
    fn allineamento_toglie_il_coperto_e_tiene_i_tempi() {
        let mut h = hybrid();
        h.pending = "CQ TEST".bytes().map(|c| (c, 1.0)).collect();
        let mut out = Vec::new();
        h.settle(b"CQ", 2.0, &mut out);
        assert_eq!(h.provisional(), " TEST");
        assert!(out.iter().all(|s| s.at_s == 1.0));
        assert_eq!(h.stats().confirmed, 2);
    }

    #[test]
    fn carattere_corretto_prende_il_tempo_definitivo() {
        let mut h = hybrid();
        h.pending = "IU3ABD K".bytes().map(|c| (c, 1.0)).collect();
        let mut out = Vec::new();
        h.settle(b"IU3ABC", 2.0, &mut out);
        assert_eq!(h.provisional(), " K");
        assert_eq!(out[5], Shown { ch: b'C', at_s: 2.0 });
        assert_eq!(out[0].at_s, 1.0);
    }

    #[test]
    fn provvisorio_inserito_si_scarta() {
        let mut h = hybrid();
        h.pending = "EE5NN".bytes().map(|c| (c, 1.0)).collect();
        let mut out = Vec::new();
        h.settle(b"5NN", 2.0, &mut out);
        assert_eq!(h.provisional(), "");
        assert!(out.iter().all(|s| s.at_s == 1.0));
    }

    #[test]
    fn definitivo_senza_provvisorio() {
        let mut h = hybrid();
        let mut out = Vec::new();
        h.settle(b"TU", 2.0, &mut out);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|s| s.at_s == 2.0));
    }

    #[test]
    fn rumore_non_lascia_provvisorio() {
        let mut h = hybrid();
        let mut seed = 1u32;
        let audio: Vec<f32> = (0..12_000 * 6)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        for b in audio.chunks(1200) {
            h.process(b);
        }
        assert!(h.provisional().trim().len() <= 2, "{:?}", h.provisional());
    }
}
