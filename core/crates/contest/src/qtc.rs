//! I QTC del WAE DX Contest (`docs/contest-qtc.md`): chi li puo' scambiare, quali QSO del log si possono ancora mandare a una stazione,
//! il numero della prossima serie e le righe `QTC:` del Cabrillo.
//!
//! Un QTC e' una riga (ora, nominativo, numero ricevuto) di un QSO del contest passata a un'altra stazione dentro una serie `QTC nn/mm`.
//! Un QSO si passa una volta sola e mai alla stazione con cui e' stato fatto; fra due stazioni al massimo 10 QTC in tutto.

use crate::calendario::{data, hhmm};
use crate::punti_testo::{QsoTesto, modo_cabrillo};
use serde::{Deserialize, Serialize};

/// Quanti QTC al massimo fra due stazioni.
pub const MAX_PER_STAZIONE: u32 = 10;

/// Come si scambiano i QTC nel contest (`m_iQtc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RegolaQtc {
    /// CW e SSB: dalle stazioni non europee a quelle europee.
    Classica,
    /// RTTY: in entrambe le direzioni, fra continenti diversi.
    Rtty,
}

impl RegolaQtc {
    pub fn da_codice(n: i32) -> Option<RegolaQtc> {
        match n {
            1 => Some(RegolaQtc::Classica),
            2 => Some(RegolaQtc::Rtty),
            _ => None,
        }
    }
}

/// Un QTC mandato o ricevuto.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Qtc {
    /// `true` mandato da noi, `false` ricevuto.
    pub inviato: bool,
    /// La stazione con cui si e' scambiata la serie.
    pub corrispondente: String,
    /// Numero della serie (di chi la manda) e quante righe contiene.
    pub serie: u32,
    pub quanti: u32,
    /// Istante della serie (ms UTC), banda (`20m`), modo e frequenza della radio.
    pub time_ms: i64,
    pub band: String,
    pub mode: String,
    pub freq_hz: u64,
    /// La riga: ora `HHMM`, nominativo e numero del QSO riportato.
    pub ora: String,
    pub call: String,
    pub nr: String,
    /// Per i QTC mandati: l'istante del QSO del log (per non mandarlo due volte).
    pub qso_ms: i64,
}

fn norm(c: &str) -> String {
    c.trim().to_ascii_uppercase()
}

/// Perche' i QTC non si possono scambiare con una stazione.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Divieto {
    /// RTTY: stesso continente.
    StessoContinente,
    /// CW e SSB: si mandano solo da fuori Europa verso l'Europa.
    DirezioneSbagliata,
    /// I 10 QTC con questa stazione sono gia' stati scambiati.
    Esauriti,
}

/// Si possono scambiare QTC in questa direzione? I continenti sconosciuti (vuoti) non bloccano. `inviare` = siamo noi a mandarli.
pub fn ammesso(regola: RegolaQtc, inviare: bool, mio_cont: &str, suo_cont: &str) -> Result<(), Divieto> {
    let (m, s) = (norm(mio_cont), norm(suo_cont));
    if m.is_empty() || s.is_empty() {
        return Ok(());
    }
    match regola {
        RegolaQtc::Rtty if m == s => Err(Divieto::StessoContinente),
        RegolaQtc::Rtty => Ok(()),
        RegolaQtc::Classica => {
            let (chi_manda, chi_riceve) = if inviare { (&m, &s) } else { (&s, &m) };
            if chi_manda != "EU" && chi_riceve == "EU" { Ok(()) } else { Err(Divieto::DirezioneSbagliata) }
        }
    }
}

/// Quanti QTC sono gia' stati scambiati con una stazione (mandati e ricevuti).
pub fn scambiati(qtc: &[Qtc], call: &str) -> u32 {
    let c = norm(call);
    qtc.iter().filter(|q| norm(&q.corrispondente) == c).count() as u32
}

/// Quanti QTC si possono ancora scambiare con una stazione.
pub fn residuo(qtc: &[Qtc], call: &str) -> u32 {
    MAX_PER_STAZIONE.saturating_sub(scambiati(qtc, call))
}

/// Il numero della prossima serie da mandare: dopo la piu' alta gia' mandata.
pub fn prossima_serie(qtc: &[Qtc]) -> u32 {
    qtc.iter().filter(|q| q.inviato).map(|q| q.serie).max().unwrap_or(0) + 1
}

/// Il numero ricevuto di un QSO: la prima parola di sole cifre che non sia il rapporto (599, 59, 5NN), con almeno tre cifre.
pub fn numero_ricevuto(exch_rcvd: &str) -> Option<String> {
    let parole: Vec<&str> = exch_rcvd.split_whitespace().collect();
    let rst = |p: &str| matches!(p.len(), 2 | 3) && p.starts_with('5') && p.bytes().all(|b| b.is_ascii_digit()) && parole.len() > 1;
    let n = parole.iter().find(|p| p.bytes().all(|b| b.is_ascii_digit()) && !p.is_empty() && !rst(p))?;
    let v: u32 = n.parse().ok()?;
    Some(format!("{v:03}"))
}

/// Una riga proposta da mandare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Proposta {
    pub ora: String,
    pub call: String,
    pub nr: String,
    pub qso_ms: i64,
}

/// I QSO del contest (validi, dal piu' vecchio) che si possono ancora mandare a `dest`: non fatti con lui, non gia' mandati, con il
/// numero ricevuto; al massimo quanti ne restano dei 10.
pub fn proposte(qsos: &[QsoTesto], qtc: &[Qtc], dest: &str) -> Vec<Proposta> {
    let d = norm(dest);
    let max = residuo(qtc, &d) as usize;
    let gia: std::collections::HashSet<(String, i64)> = qtc.iter().filter(|q| q.inviato).map(|q| (norm(&q.call), q.qso_ms)).collect();
    qsos.iter()
        .filter(|q| norm(&q.call) != d && !d.is_empty())
        .filter(|q| !gia.contains(&(norm(&q.call), q.time_ms)))
        .filter_map(|q| numero_ricevuto(&q.exch_rcvd).map(|nr| Proposta { ora: hhmm(q.time_ms), call: norm(&q.call), nr, qso_ms: q.time_ms }))
        .take(max)
        .collect()
}

/// Controlla una serie prima di salvarla: righe complete, serie valida, non piu' di quante ne restano con la stazione, nessuna riga
/// che torni alla stazione da cui viene.
pub fn controlla(nuovi: &[Qtc], esistenti: &[Qtc]) -> Result<(), String> {
    for q in nuovi {
        if norm(&q.corrispondente).is_empty() || norm(&q.call).is_empty() || q.ora.trim().len() != 4 || q.nr.trim().is_empty() {
            return Err("riga QTC incompleta".into());
        }
        if q.serie == 0 || q.quanti == 0 || q.quanti > MAX_PER_STAZIONE {
            return Err("serie QTC non valida".into());
        }
        if norm(&q.call) == norm(&q.corrispondente) {
            return Err("un QTC non torna alla stazione da cui viene".into());
        }
    }
    let mut tutti = esistenti.to_vec();
    for q in nuovi {
        tutti.push(q.clone());
        if scambiati(&tutti, &q.corrispondente) > MAX_PER_STAZIONE {
            return Err(format!("con {} si arriva oltre i {MAX_PER_STAZIONE} QTC", norm(&q.corrispondente)));
        }
    }
    Ok(())
}

/// I QTC dentro la finestra del contest.
pub fn nella_finestra(qtc: &[Qtc], finestra: Option<(Option<i64>, Option<i64>)>) -> Vec<Qtc> {
    qtc.iter()
        .filter(|q| match finestra {
            None => true,
            Some((da, a)) => da.is_none_or(|x| q.time_ms >= x) && a.is_none_or(|x| q.time_ms <= x),
        })
        .cloned()
        .collect()
}

/// Le righe `QTC:` del Cabrillo (`QRG MODE DATE TIME CALL-RX QTC-GRP CALL-TX TIME-QSO CALL-QSO NR-QSO`), in ordine di tempo.
pub fn righe_cabrillo(qtc: &[Qtc], mio: &str) -> String {
    let mio = norm(mio);
    let mut v: Vec<&Qtc> = qtc.iter().collect();
    v.sort_by_key(|q| q.time_ms);
    let mut out = String::new();
    for q in v {
        let khz = q.freq_hz / 1000;
        let freq = if khz > 0 { khz.to_string() } else { crate::geo::banda_cabrillo(&q.band) };
        let suo = norm(&q.corrispondente);
        let (rx, tx) = if q.inviato { (suo.as_str(), mio.as_str()) } else { (mio.as_str(), suo.as_str()) };
        let grp = format!("{}/{}", q.serie, q.quanti);
        out.push_str(&format!(
            "QTC: {freq:>5} {} {} {} {rx:<13} {grp:<5} {tx:<13} {} {:<13} {}\r\n",
            modo_cabrillo(&q.mode),
            data(q.time_ms),
            hhmm(q.time_ms),
            q.ora.trim(),
            norm(&q.call),
            q.nr.trim()
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elenco::data_ms;

    fn qso(call: &str, min: i64, rcvd: &str) -> QsoTesto {
        QsoTesto {
            call: call.into(),
            band: "20m".into(),
            mode: "RTTY".into(),
            freq_hz: 14_085_000,
            time_ms: data_ms("20261114120000").unwrap_or(0) + min * 60_000,
            rst_sent: "599".into(),
            rst_rcvd: "599".into(),
            exch_sent: String::new(),
            exch_rcvd: rcvd.into(),
        }
    }

    fn inviato(dest: &str, serie: u32, call: &str, qso_ms: i64) -> Qtc {
        Qtc { inviato: true, corrispondente: dest.into(), serie, quanti: 1, ora: "1200".into(), call: call.into(), nr: "001".into(), qso_ms, ..Qtc::default() }
    }

    #[test]
    fn chi_puo_scambiare() {
        assert_eq!(ammesso(RegolaQtc::Rtty, true, "EU", "EU"), Err(Divieto::StessoContinente));
        assert_eq!(ammesso(RegolaQtc::Rtty, true, "EU", "NA"), Ok(()));
        assert_eq!(ammesso(RegolaQtc::Rtty, false, "EU", "NA"), Ok(()));
        assert_eq!(ammesso(RegolaQtc::Rtty, true, "NA", "SA"), Ok(()));
        assert_eq!(ammesso(RegolaQtc::Classica, true, "NA", "EU"), Ok(()));
        assert_eq!(ammesso(RegolaQtc::Classica, true, "EU", "NA"), Err(Divieto::DirezioneSbagliata));
        assert_eq!(ammesso(RegolaQtc::Classica, false, "EU", "NA"), Ok(()));
        assert_eq!(ammesso(RegolaQtc::Rtty, true, "", "EU"), Ok(()));
    }

    #[test]
    fn numero_dal_ricevuto() {
        assert_eq!(numero_ricevuto("599 015"), Some("015".into()));
        assert_eq!(numero_ricevuto("15"), Some("015".into()));
        assert_eq!(numero_ricevuto("599 1234"), Some("1234".into()));
        assert_eq!(numero_ricevuto(""), None);
        assert_eq!(numero_ricevuto("MI"), None);
    }

    #[test]
    fn proposte_e_limiti() {
        let log = [qso("K1ABC", 0, "599 001"), qso("W2XYZ", 1, "599 002"), qso("VE3AAA", 2, ""), qso("N4BBB", 3, "599 010")];
        // a K1ABC non torna il suo QSO, e quello senza numero non si propone
        let p = proposte(&log, &[], "k1abc");
        assert_eq!(p.iter().map(|x| x.call.as_str()).collect::<Vec<_>>(), vec!["W2XYZ", "N4BBB"]);
        assert_eq!(p[0].ora, "1201");
        assert_eq!(p[0].nr, "002");
        // gia' mandato (anche ad altri): non si ripropone
        let fatti = [inviato("DL1ABC", 1, "W2XYZ", log[1].time_ms)];
        let p = proposte(&log, &fatti, "K1ABC");
        assert_eq!(p.iter().map(|x| x.call.as_str()).collect::<Vec<_>>(), vec!["N4BBB"]);
        assert_eq!(prossima_serie(&fatti), 2);
        assert_eq!(prossima_serie(&[]), 1);
        // 9 gia' scambiati con K1ABC: ne resta uno
        let nove: Vec<Qtc> = (0..9).map(|i| Qtc { corrispondente: "K1ABC".into(), serie: 1, ..inviato("K1ABC", 1, "X", i) }).collect();
        assert_eq!(residuo(&nove, "k1abc"), 1);
        assert_eq!(proposte(&log, &nove, "K1ABC").len(), 1);
    }

    #[test]
    fn controllo_della_serie() {
        let ok = Qtc { quanti: 2, ..inviato("K1ABC", 1, "W2XYZ", 5) };
        assert!(controlla(std::slice::from_ref(&ok), &[]).is_ok());
        let torna = Qtc { call: "K1ABC".into(), ..ok.clone() };
        assert!(controlla(&[torna], &[]).is_err());
        let vuota = Qtc { nr: String::new(), ..ok.clone() };
        assert!(controlla(&[vuota], &[]).is_err());
        let dieci: Vec<Qtc> = (0..10).map(|i| inviato("K1ABC", 1, "X", i)).collect();
        assert!(controlla(&[ok], &dieci).is_err());
    }

    #[test]
    fn righe_del_cabrillo() {
        let t = data_ms("20261114102800").unwrap_or(0);
        let mandato = Qtc { inviato: true, corrispondente: "DL1XYZ".into(), serie: 7, quanti: 10, time_ms: t, band: "15m".into(), mode: "RTTY".into(), freq_hz: 21_011_000, ora: "1332".into(), call: "S59XYZ".into(), nr: "112".into(), qso_ms: 0 };
        let ricevuto = Qtc { inviato: false, corrispondente: "K5XYZ".into(), time_ms: t + 60_000, ..mandato.clone() };
        let r = righe_cabrillo(&[ricevuto, mandato], "iu3abc");
        let righe: Vec<&str> = r.lines().collect();
        assert_eq!(righe[0], "QTC: 21011 RY 2026-11-14 1028 DL1XYZ        7/10  IU3ABC        1332 S59XYZ        112");
        assert_eq!(righe[1], "QTC: 21011 RY 2026-11-14 1029 IU3ABC        7/10  K5XYZ         1332 S59XYZ        112");
    }

    #[test]
    fn finestra_dei_qtc() {
        let a = Qtc { time_ms: 100, ..Qtc::default() };
        let b = Qtc { time_ms: 300, ..Qtc::default() };
        assert_eq!(nella_finestra(&[a.clone(), b], Some((Some(50), Some(200)))), vec![a]);
    }
}
