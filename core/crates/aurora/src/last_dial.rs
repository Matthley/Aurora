//! L'ultima frequenza della radio e il suo modo (USB, LSB, PKTUSB...), ricordati tra un avvio e l'altro in `ultima-frequenza.json`.
//!
//! Richiesta del proprietario (2026-11-14): riaprendo Aurora la radio deve tornare esattamente dov'era (per esempio 21,090 in RTTY nella pagina
//! dei modi testuali), non sulla frequenza FT8 principale dell'ultima banda. La frequenza si scrive solo quando cambia e solo dopo che e'
//! rimasta ferma per un po' (chi la scrive la legge ogni pochi secondi), cosi' girando la manopola non si scrive di continuo.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const LAST_DIAL_FILE: &str = "ultima-frequenza.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LastDial {
    pub dial_hz: u64,
    /// Il modo della radio come lo riporta il CAT (`USB`, `LSB`, `PKTUSB`...); assente se il CAT non lo dice (OmniRig).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub radio_mode: Option<String>,
}

impl LastDial {
    pub fn load(dir: &Path) -> Option<LastDial> {
        let bytes = std::fs::read(dir.join(LAST_DIAL_FILE)).ok()?;
        serde_json::from_slice::<LastDial>(&bytes).ok().filter(|d| d.dial_hz > 0)
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(dir.join(LAST_DIAL_FILE), json)
    }

    /// Cosa mandare alla radio all'avvio: la frequenza se e' diversa da quella di adesso, il modo se e' noto e diverso. Niente se la radio
    /// e' gia' li'.
    pub fn restore_plan(&self, now_hz: u64, now_mode: Option<&str>) -> (Option<u64>, Option<String>) {
        let freq = (self.dial_hz > 0 && self.dial_hz != now_hz).then_some(self.dial_hz);
        let mode = self.radio_mode.as_deref().filter(|m| !m.is_empty() && now_mode.is_none_or(|n| !n.eq_ignore_ascii_case(m))).map(str::to_string);
        (freq, mode)
    }
}

/// Lo scrittore con l'attesa: una lettura nuova si scrive solo dopo che e' rimasta uguale per `steady` letture di fila.
#[derive(Debug, Default)]
pub struct DialRecorder {
    saved: Option<LastDial>,
    candidate: Option<LastDial>,
    seen: u32,
}

impl DialRecorder {
    pub fn new(saved: Option<LastDial>) -> DialRecorder {
        DialRecorder { saved, candidate: None, seen: 0 }
    }

    /// Una lettura della radio; restituisce cosa scrivere (se c'e' da scrivere).
    pub fn observe(&mut self, reading: LastDial, steady: u32) -> Option<LastDial> {
        if reading.dial_hz == 0 {
            return None;
        }
        // il modo non letto (OmniRig, lettura mancata) non cancella quello ricordato
        let reading = match (&reading.radio_mode, &self.saved) {
            (None, Some(s)) if s.dial_hz == reading.dial_hz || s.radio_mode.is_some() => LastDial { radio_mode: s.radio_mode.clone(), ..reading },
            _ => reading,
        };
        if self.candidate.as_ref() == Some(&reading) {
            self.seen += 1;
        } else {
            self.candidate = Some(reading.clone());
            self.seen = 1;
        }
        if self.seen >= steady.max(1) && self.saved.as_ref() != Some(&reading) {
            self.saved = Some(reading.clone());
            return Some(reading);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dial(hz: u64, mode: Option<&str>) -> LastDial {
        LastDial { dial_hz: hz, radio_mode: mode.map(str::to_string) }
    }

    #[test]
    fn restore_sends_only_what_differs() {
        let saved = dial(21_090_000, Some("USB"));
        assert_eq!(saved.restore_plan(28_074_000, Some("PKTUSB")), (Some(21_090_000), Some("USB".into())));
        assert_eq!(saved.restore_plan(21_090_000, Some("usb")), (None, None));
        assert_eq!(saved.restore_plan(21_090_000, None), (None, Some("USB".into())));
        assert_eq!(dial(7_040_000, None).restore_plan(7_074_000, Some("LSB")), (Some(7_040_000), None));
    }

    #[test]
    fn a_new_frequency_is_written_only_once_it_stays_still() {
        let mut rec = DialRecorder::new(Some(dial(14_074_000, Some("PKTUSB"))));
        assert_eq!(rec.observe(dial(14_074_000, Some("PKTUSB")), 2), None, "uguale a quella salvata");
        assert_eq!(rec.observe(dial(21_080_000, Some("USB")), 2), None, "ancora in movimento");
        assert_eq!(rec.observe(dial(21_090_000, Some("USB")), 2), None);
        assert_eq!(rec.observe(dial(21_090_000, Some("USB")), 2), Some(dial(21_090_000, Some("USB"))));
        assert_eq!(rec.observe(dial(21_090_000, Some("USB")), 2), None, "gia' scritta");
        assert_eq!(rec.observe(dial(0, None), 2), None, "radio che non risponde");
    }

    #[test]
    fn a_missing_mode_keeps_the_remembered_one() {
        let mut rec = DialRecorder::new(Some(dial(3_580_000, Some("LSB"))));
        assert_eq!(rec.observe(dial(3_590_000, None), 1), Some(dial(3_590_000, Some("LSB"))));
    }

    #[test]
    fn saved_file_round_trips() {
        let dir = std::env::temp_dir().join(format!("aurora-last-dial-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dial(21_090_000, Some("USB")).save(&dir).unwrap();
        assert_eq!(LastDial::load(&dir), Some(dial(21_090_000, Some("USB"))));
        std::fs::write(dir.join(LAST_DIAL_FILE), b"{\"dial_hz\":0}").unwrap();
        assert_eq!(LastDial::load(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
