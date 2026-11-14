//! Punteggio e Cabrillo del contest attivo dei modi testuali (file `.contest`, `docs/contest-file.md`).
//!
//! I QSO sono quelli del log salvati dalla pagina dei modi testuali: gli scambi stanno in coda al commento (`NRS 001 NRR 015`).
//! Contano i QSO dentro la finestra del contest (le date del file portate nell'anno corrente) e di una categoria valida (CW, fonia,
//! digitale; nessuna scelta = tutte). Un doppio (stesso nominativo, stessa banda salvo `m_bDupeNoBand`, stesso modo salvo
//! `m_bDupeNoMode`) vale zero punti e nessun moltiplicatore.
//!
//! Punti: `m_iPointsMyDxcc` per lo stesso paese, `m_iPointsMyCont` per lo stesso continente, `m_iPointsDx` per gli altri; tutti a zero
//! = un punto per QSO. Moltiplicatori: `m_iMultipliers` e `m_iMultipliersOption` (si sommano), ognuno 0 nessuno, 1 paese DXCC,
//! 2 prefisso WPX, 3 zona CQ, 4 zona ITU, 5 dominio (dall'elenco `.dom`, o lo scambio ricevuto se l'elenco manca). Con `m_bBandMult`
//! i moltiplicatori si contano per banda e quelli di una banda accesa con peso valgono il peso. Punteggio = punti x moltiplicatori
//! (senza moltiplicatori: i punti).

use crate::calendario::{civil_from_days, data, days_from_civil, hhmm};
use crate::elenco::{ContestTesto, Dominio, data_ms};
use crate::geo;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

/// Un QSO del log per il contest dei modi testuali.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct QsoTesto {
    pub call: String,
    /// `20m`.
    pub band: String,
    pub mode: String,
    pub freq_hz: u64,
    /// Inizio del QSO, ms UTC.
    pub time_ms: i64,
    pub rst_sent: String,
    pub rst_rcvd: String,
    pub exch_sent: String,
    pub exch_rcvd: String,
}

/// Paese, continente e zone di un nominativo (cty.dat).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InfoPaese {
    /// Il prefisso principale del paese DXCC.
    pub dxcc: String,
    pub continente: String,
    pub cq: u8,
    pub itu: u8,
}

/// Gli scambi scritti in coda al commento dalla pagina dei modi testuali: `note NRS 001 NRR 015` -> (`001`, `015`).
pub fn scambi_da_commento(commento: &str) -> (String, String) {
    let parole: Vec<&str> = commento.split_whitespace().collect();
    let Some(i) = parole.iter().rposition(|p| *p == "NRS") else { return (String::new(), String::new()) };
    let resto = &parole[i + 1..];
    let Some(j) = resto.iter().position(|p| *p == "NRR") else { return (resto.join(" "), String::new()) };
    (resto[..j].join(" "), resto[j + 1..].join(" "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Categoria {
    Cw,
    Phone,
    Digital,
}

/// La categoria di un modo: CW, fonia (SSB, AM, FM) o digitale (tutto il resto).
pub fn categoria(modo: &str) -> Categoria {
    match modo.trim().to_ascii_uppercase().as_str() {
        "CW" => Categoria::Cw,
        "SSB" | "USB" | "LSB" | "AM" | "FM" | "PHONE" => Categoria::Phone,
        _ => Categoria::Digital,
    }
}

/// Il modo conta per il contest? Nessuna categoria scelta = tutte.
pub fn categoria_valida(c: &ContestTesto, modo: &str) -> bool {
    if !c.valid_cw && !c.valid_phone && !c.valid_digital {
        return true;
    }
    match categoria(modo) {
        Categoria::Cw => c.valid_cw,
        Categoria::Phone => c.valid_phone,
        Categoria::Digital => c.valid_digital,
    }
}

fn anno_di(ms: i64) -> i64 {
    civil_from_days(ms.div_euclid(86_400_000)).0
}

fn sposta_anni(ms: i64, anni: i64) -> i64 {
    let giorno = ms.div_euclid(86_400_000);
    let (y, m, d) = civil_from_days(giorno);
    days_from_civil(y + anni, m, d) * 86_400_000 + ms.rem_euclid(86_400_000)
}

/// La finestra del contest nell'anno di `ora_ms` (le date del file valgono ogni anno), come nella pagina. `None` = nessuna data.
pub fn finestra(c: &ContestTesto, ora_ms: i64) -> Option<(Option<i64>, Option<i64>)> {
    let (a, b) = (data_ms(&c.start), data_ms(&c.end));
    let rif = a.or(b)?;
    let anni = anno_di(ora_ms) - anno_di(rif);
    Some((a.map(|x| sposta_anni(x, anni)), b.map(|x| sposta_anni(x, anni))))
}

/// I QSO che contano per il contest (finestra e categoria), dal piu' vecchio.
pub fn qso_del_contest(c: &ContestTesto, qsos: &[QsoTesto], ora_ms: i64) -> Vec<QsoTesto> {
    let w = finestra(c, ora_ms);
    let mut v: Vec<QsoTesto> = qsos
        .iter()
        .filter(|q| categoria_valida(c, &q.mode))
        .filter(|q| match w {
            None => true,
            Some((da, a)) => da.is_none_or(|x| q.time_ms >= x) && a.is_none_or(|x| q.time_ms <= x),
        })
        .cloned()
        .collect();
    v.sort_by_key(|q| q.time_ms);
    v
}

/// Il tipo di moltiplicatore (`m_iMultipliers`, `m_iMultipliersOption`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoMolt {
    Dxcc,
    Wpx,
    Cq,
    Itu,
    Dominio,
}

impl TipoMolt {
    pub fn da_codice(n: i32) -> Option<TipoMolt> {
        match n {
            1 => Some(TipoMolt::Dxcc),
            2 => Some(TipoMolt::Wpx),
            3 => Some(TipoMolt::Cq),
            4 => Some(TipoMolt::Itu),
            5 => Some(TipoMolt::Dominio),
            _ => None,
        }
    }
}

/// I tipi di moltiplicatore del contest (senza ripetizioni).
pub fn tipi_molt(c: &ContestTesto) -> Vec<TipoMolt> {
    let mut v: Vec<TipoMolt> = [c.multipliers, c.multipliers_option].into_iter().filter_map(TipoMolt::da_codice).collect();
    v.dedup();
    v
}

/// Il peso dei moltiplicatori di una banda (`20m` o `20`): quello del file se la banda e' accesa e il peso e' positivo, altrimenti 1.
pub fn peso_banda(c: &ContestTesto, banda: &str) -> i64 {
    let b = banda.trim().trim_end_matches(['m', 'M']);
    c.bands.iter().find(|x| x.band == b && x.on && x.weight > 0).map_or(1, |x| i64::from(x.weight))
}

/// Un QSO valutato.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Valutato {
    pub qso: QsoTesto,
    pub doppio: bool,
    pub punti: i64,
    /// I moltiplicatori nuovi portati da questo QSO (`dxcc:I`, `cq:15`...).
    pub nuovi: Vec<String>,
}

/// Una riga della statistica per banda.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct RigaBanda {
    pub band: String,
    pub qso: u32,
    pub doppi: u32,
    pub punti: i64,
    pub molt: u32,
    pub cw: u32,
    pub phone: u32,
    pub digital: u32,
}

/// Il punteggio del contest.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Punteggio {
    /// QSO validi (senza i doppi) e doppi.
    pub qso: u32,
    pub doppi: u32,
    pub punti: i64,
    /// I moltiplicatori (somma dei pesi con i moltiplicatori per banda).
    pub molt: i64,
    pub punteggio: i64,
    /// I tipi di moltiplicatore usati.
    pub tipi: Vec<TipoMolt>,
    pub bande: Vec<RigaBanda>,
}

/// Il valore di un moltiplicatore per un QSO (`None` se non si sa).
fn valore_molt(t: TipoMolt, q: &QsoTesto, info: Option<&InfoPaese>, dominio: Option<&Dominio>) -> Option<String> {
    let v = match t {
        TipoMolt::Dxcc => info.map(|i| i.dxcc.clone()).unwrap_or_default(),
        TipoMolt::Wpx => geo::prefisso_wpx(&q.call),
        TipoMolt::Cq => info.filter(|i| i.cq > 0).map(|i| i.cq.to_string()).unwrap_or_default(),
        TipoMolt::Itu => info.filter(|i| i.itu > 0).map(|i| i.itu.to_string()).unwrap_or_default(),
        TipoMolt::Dominio => {
            let parole: Vec<&str> = q.exch_rcvd.split_whitespace().collect();
            match dominio {
                Some(d) => parole.iter().find_map(|p| d.cerca(p, &q.call)).unwrap_or_default(),
                // senza elenco: l'ultima parola che non e' un numero (provincia, stato), altrimenti l'ultima
                None => parole.iter().rev().find(|p| !p.bytes().all(|b| b.is_ascii_digit())).or(parole.last()).map(|p| p.to_ascii_uppercase()).unwrap_or_default(),
            }
        }
    };
    let v = v.trim().to_ascii_uppercase();
    (!v.is_empty()).then_some(v)
}

fn nome_tipo(t: TipoMolt) -> &'static str {
    match t {
        TipoMolt::Dxcc => "dxcc",
        TipoMolt::Wpx => "wpx",
        TipoMolt::Cq => "cq",
        TipoMolt::Itu => "itu",
        TipoMolt::Dominio => "dom",
    }
}

/// Valuta i QSO del contest (gia' filtrati da [`qso_del_contest`], dal piu' vecchio): doppi, punti, moltiplicatori, punteggio e
/// statistica per banda. `mio` = paese della stazione, `paese` = la ricerca nel cty.dat.
pub fn valuta(c: &ContestTesto, qsos: &[QsoTesto], mio: Option<&InfoPaese>, paese: &dyn Fn(&str) -> Option<InfoPaese>, dominio: Option<&Dominio>) -> (Vec<Valutato>, Punteggio) {
    let tipi = tipi_molt(c);
    let tutti_zero = c.points_my_dxcc == 0 && c.points_my_cont == 0 && c.points_dx == 0;
    let mut visti: HashSet<(String, String, String)> = HashSet::new();
    let mut molt: HashSet<String> = HashSet::new();
    let mut bande: BTreeMap<String, RigaBanda> = BTreeMap::new();
    let mut out = Vec::with_capacity(qsos.len());
    let mut tot = Punteggio { tipi: tipi.clone(), ..Punteggio::default() };
    for q in qsos {
        let call = q.call.trim().to_ascii_uppercase();
        let chiave = (call.clone(), if c.dupe_no_band { String::new() } else { q.band.to_ascii_lowercase() }, if c.dupe_no_mode { String::new() } else { q.mode.to_ascii_uppercase() });
        let doppio = !visti.insert(chiave);
        let riga = bande.entry(q.band.to_ascii_lowercase()).or_insert_with(|| RigaBanda { band: q.band.to_ascii_lowercase(), ..RigaBanda::default() });
        if doppio {
            tot.doppi += 1;
            riga.doppi += 1;
            out.push(Valutato { qso: q.clone(), doppio: true, punti: 0, nuovi: Vec::new() });
            continue;
        }
        let info = paese(&call);
        let punti = if tutti_zero {
            1
        } else {
            match (mio, info.as_ref()) {
                (Some(m), Some(i)) if !m.dxcc.is_empty() && m.dxcc == i.dxcc => i64::from(c.points_my_dxcc),
                (Some(m), Some(i)) if !m.continente.is_empty() && m.continente == i.continente => i64::from(c.points_my_cont),
                _ => i64::from(c.points_dx),
            }
        };
        let mut nuovi = Vec::new();
        for t in &tipi {
            if let Some(v) = valore_molt(*t, q, info.as_ref(), dominio) {
                let k = if c.band_mult { format!("{}:{v}@{}", nome_tipo(*t), q.band.to_ascii_lowercase()) } else { format!("{}:{v}", nome_tipo(*t)) };
                if molt.insert(k) {
                    nuovi.push(format!("{}:{v}", nome_tipo(*t)));
                    riga.molt += 1;
                    tot.molt += if c.band_mult { peso_banda(c, &q.band) } else { 1 };
                }
            }
        }
        tot.qso += 1;
        tot.punti += punti;
        riga.qso += 1;
        riga.punti += punti;
        match categoria(&q.mode) {
            Categoria::Cw => riga.cw += 1,
            Categoria::Phone => riga.phone += 1,
            Categoria::Digital => riga.digital += 1,
        }
        out.push(Valutato { qso: q.clone(), doppio: false, punti, nuovi });
    }
    tot.punteggio = if tipi.is_empty() { tot.punti } else { tot.punti * tot.molt };
    tot.bande = bande.into_values().collect();
    tot.bande.sort_by_key(|r| ordine_banda(&r.band));
    (out, tot)
}

/// Le bande in ordine di frequenza (le sconosciute in fondo).
fn ordine_banda(b: &str) -> u32 {
    const ORDINE: [&str; 13] = ["160m", "80m", "60m", "40m", "30m", "20m", "17m", "15m", "12m", "10m", "6m", "2m", "70cm"];
    ORDINE.iter().position(|x| *x == b).map_or(99, |i| i as u32)
}

/// Il modo come lo vuole il Cabrillo: `CW`, `RY` (RTTY), `PH` (fonia), `DG` (gli altri digitali).
pub fn modo_cabrillo(modo: &str) -> &'static str {
    match modo.trim().to_ascii_uppercase().as_str() {
        "RTTY" => "RY",
        m => match categoria(m) {
            Categoria::Cw => "CW",
            Categoria::Phone => "PH",
            Categoria::Digital => "DG",
        },
    }
}

/// `CATEGORY-MODE` dai QSO: un solo tipo -> `CW`, `RTTY`, `SSB`, `DIGI`; piu' tipi -> `MIXED`.
fn categoria_modo(log: &[Valutato]) -> &'static str {
    let tipi: HashSet<&str> = log.iter().map(|v| modo_cabrillo(&v.qso.mode)).collect();
    if tipi.len() != 1 {
        return if tipi.is_empty() { "DIGI" } else { "MIXED" };
    }
    match tipi.into_iter().next() {
        Some("CW") => "CW",
        Some("RY") => "RTTY",
        Some("PH") => "SSB",
        _ => "DIGI",
    }
}

/// Lo scambio di una colonna: il rapporto davanti (se c'e' e lo scambio non lo contiene gia') e lo scambio; `-` se vuoto.
fn scambio(rst: &str, ex: &str) -> String {
    let (rst, ex) = (rst.trim().to_ascii_uppercase(), ex.trim().to_ascii_uppercase());
    let s = if rst.is_empty() || ex.split_whitespace().next() == Some(rst.as_str()) { ex } else { format!("{rst} {ex}") };
    let s = s.trim().to_string();
    if s.is_empty() { crate::export::MANCA.to_string() } else { s }
}

/// Il nome del contest per la riga `CONTEST:` (maiuscole, spazi in trattini).
fn nome_cabrillo(nome: &str) -> String {
    nome.split_whitespace().collect::<Vec<_>>().join("-").to_ascii_uppercase()
}

/// Il file Cabrillo 3.0 del contest dei modi testuali. I doppi restano nel file (li toglie chi controlla i log).
pub fn cabrillo(c: &ContestTesto, call: &str, log: &[Valutato], tot: &Punteggio, versione: &str) -> String {
    let mut out = String::new();
    let mut riga = |k: &str, v: &str| {
        if !v.trim().is_empty() {
            out.push_str(&format!("{k}: {}\r\n", v.trim()));
        }
    };
    let call = call.trim().to_ascii_uppercase();
    riga("START-OF-LOG", "3.0");
    riga("CONTEST", &nome_cabrillo(&c.name));
    riga("CALLSIGN", &call);
    riga("CATEGORY-OPERATOR", "SINGLE-OP");
    riga("CATEGORY-BAND", "ALL");
    riga("CATEGORY-MODE", categoria_modo(log));
    riga("CATEGORY-TRANSMITTER", "ONE");
    riga("CLAIMED-SCORE", &tot.punteggio.to_string());
    riga("CREATED-BY", &format!("Aurora {versione}"));
    for v in log {
        let khz = v.qso.freq_hz / 1000;
        let freq = if khz > 0 { khz.to_string() } else { geo::banda_cabrillo(&v.qso.band) };
        out.push_str(&format!(
            "QSO: {freq:>5} {} {} {} {:<13} {:<10} {:<13} {}\r\n",
            modo_cabrillo(&v.qso.mode),
            data(v.qso.time_ms),
            hhmm(v.qso.time_ms),
            call,
            scambio(&v.qso.rst_sent, &v.qso.exch_sent),
            v.qso.call.trim().to_ascii_uppercase(),
            scambio(&v.qso.rst_rcvd, &v.qso.exch_rcvd)
        ));
    }
    out.push_str("END-OF-LOG:\r\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::elenco::leggi_dominio;

    fn q(call: &str, band: &str, mode: &str, min: i64, rcvd: &str) -> QsoTesto {
        QsoTesto {
            call: call.into(),
            band: band.into(),
            mode: mode.into(),
            freq_hz: if band == "20m" { 14_080_500 } else { 7_040_000 },
            time_ms: data_ms("20261114120000").unwrap_or(0) + min * 60_000,
            rst_sent: "599".into(),
            rst_rcvd: "599".into(),
            exch_sent: format!("{:03}", min + 1),
            exch_rcvd: rcvd.into(),
        }
    }

    fn paese(call: &str) -> Option<InfoPaese> {
        let (dxcc, cont, cq, itu) = match &call[..1] {
            "I" => ("I", "EU", 15, 28),
            "D" => ("DL", "EU", 14, 28),
            "W" | "K" => ("K", "NA", 5, 8),
            _ => return None,
        };
        Some(InfoPaese { dxcc: dxcc.into(), continente: cont.into(), cq, itu })
    }

    fn mio() -> InfoPaese {
        paese("IU3ABC").unwrap_or_default()
    }

    #[test]
    fn scambi_dal_commento() {
        assert_eq!(scambi_da_commento("bello NRS 001 NRR 599 015"), ("001".into(), "599 015".into()));
        assert_eq!(scambi_da_commento("nessuno"), (String::new(), String::new()));
        assert_eq!(scambi_da_commento("NRS MI NRR"), ("MI".into(), String::new()));
    }

    #[test]
    fn punti_per_paese_continente_dx_e_doppi() {
        let c = ContestTesto { points_my_dxcc: 1, points_my_cont: 2, points_dx: 3, ..ContestTesto::default() };
        let log = [q("IK2XYZ", "20m", "RTTY", 0, "1"), q("DL1ABC", "20m", "RTTY", 1, "2"), q("W1AW", "20m", "RTTY", 2, "3"), q("DL1ABC", "20m", "RTTY", 3, "4"), q("DL1ABC", "40m", "RTTY", 4, "5")];
        let (v, t) = valuta(&c, &log, Some(&mio()), &paese, None);
        assert_eq!((t.qso, t.doppi, t.punti, t.molt, t.punteggio), (4, 1, 1 + 2 + 3 + 2, 0, 8));
        assert!(v[3].doppio);
        assert_eq!(t.bande.len(), 2);
        assert_eq!(t.bande[0].band, "40m");
        // doppio su ogni banda
        let c2 = ContestTesto { dupe_no_band: true, ..c };
        let (_, t2) = valuta(&c2, &log, Some(&mio()), &paese, None);
        assert_eq!(t2.doppi, 2);
    }

    #[test]
    fn senza_punti_uno_per_qso_e_modo() {
        let c = ContestTesto::default();
        let log = [q("DL1ABC", "20m", "RTTY", 0, ""), q("DL1ABC", "20m", "CW", 1, "")];
        let (_, t) = valuta(&c, &log, None, &paese, None);
        assert_eq!((t.qso, t.punti, t.punteggio), (2, 2, 2));
        let c2 = ContestTesto { dupe_no_mode: true, ..ContestTesto::default() };
        assert_eq!(valuta(&c2, &log, None, &paese, None).1.doppi, 1);
    }

    #[test]
    fn moltiplicatori_dxcc_wpx_e_zone() {
        let log = [q("DL1ABC", "20m", "RTTY", 0, ""), q("DL2XYZ", "20m", "RTTY", 1, ""), q("W1AW", "20m", "RTTY", 2, ""), q("DL1ABC", "40m", "RTTY", 3, "")];
        let c = ContestTesto { multipliers: 1, ..ContestTesto::default() };
        let (v, t) = valuta(&c, &log, None, &paese, None);
        assert_eq!((t.punti, t.molt, t.punteggio), (4, 2, 8));
        assert_eq!(v[0].nuovi, vec!["dxcc:DL".to_string()]);
        let c = ContestTesto { multipliers: 2, multipliers_option: 3, ..ContestTesto::default() };
        let (_, t) = valuta(&c, &log, None, &paese, None);
        // WPX: DL1, DL2, W1 ; zone CQ: 14, 5
        assert_eq!(t.molt, 5);
        let c = ContestTesto { multipliers: 4, ..ContestTesto::default() };
        assert_eq!(valuta(&c, &log, None, &paese, None).1.molt, 2);
    }

    #[test]
    fn moltiplicatori_per_banda_con_peso() {
        let mut c = ContestTesto { multipliers: 1, band_mult: true, ..ContestTesto::default() };
        for b in &mut c.bands {
            if b.band == "40" {
                b.on = true;
                b.weight = 2;
            }
        }
        let log = [q("DL1ABC", "20m", "RTTY", 0, ""), q("W1AW", "20m", "RTTY", 1, ""), q("DL1ABC", "40m", "RTTY", 2, "")];
        let (_, t) = valuta(&c, &log, None, &paese, None);
        // 20m: DL, K (peso 1); 40m: DL (peso 2)
        assert_eq!((t.molt, t.punteggio), (4, 12));
        assert_eq!(peso_banda(&c, "40m"), 2);
        assert_eq!(peso_banda(&c, "20m"), 1);
    }

    #[test]
    fn moltiplicatori_dominio() {
        let d = leggi_dominio("Province\nMilano = MI\nMI\nRoma = RM\n");
        let log = [q("IK2XYZ", "20m", "RTTY", 0, "599 MILANO"), q("IK2ABC", "20m", "RTTY", 1, "599 MI"), q("IK0AAA", "20m", "RTTY", 2, "599 ROMA"), q("IK0BBB", "20m", "RTTY", 3, "599 XX")];
        let c = ContestTesto { multipliers: 5, ..ContestTesto::default() };
        assert_eq!(valuta(&c, &log, None, &paese, Some(&d)).1.molt, 2);
        // senza elenco: l'ultima parola non numerica
        assert_eq!(valuta(&c, &log, None, &paese, None).1.molt, 4);
    }

    #[test]
    fn finestra_e_categorie() {
        let c = ContestTesto { start: "20201114110000".into(), end: "20201114130000".into(), valid_cw: true, valid_digital: false, ..ContestTesto::default() };
        let ora = data_ms("20261114125900").unwrap_or(0);
        let log = [q("DL1ABC", "20m", "CW", 0, ""), q("DL2ABC", "20m", "RTTY", 1, ""), q("DL3ABC", "20m", "CW", 90, "")];
        let v = qso_del_contest(&c, &log, ora);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].call, "DL1ABC");
        let (a, b) = finestra(&c, ora).unwrap_or((None, None));
        assert_eq!(a, data_ms("20261114110000"));
        assert_eq!(b, data_ms("20261114130000"));
    }

    #[test]
    fn cabrillo_righe() {
        let c = ContestTesto { name: "Test Contest RTTY".into(), ..ContestTesto::default() };
        let log = [q("DL1ABC", "20m", "RTTY", 0, "599 015"), q("W1AW", "40m", "CW", 1, "")];
        let (v, t) = valuta(&c, &log, None, &paese, None);
        let f = cabrillo(&c, "iu3abc", &v, &t, "0.11.0");
        assert!(f.starts_with("START-OF-LOG: 3.0\r\n"));
        assert!(f.contains("CONTEST: TEST-CONTEST-RTTY\r\n"));
        assert!(f.contains("CATEGORY-MODE: MIXED\r\n"));
        assert!(f.contains("CLAIMED-SCORE: 2\r\n"));
        assert!(f.contains("QSO: 14080 RY 2026-11-14 1200 IU3ABC        599 001    DL1ABC        599 015\r\n"), "{f}");
        assert!(f.contains("QSO:  7040 CW 2026-11-14 1201 IU3ABC        599 002    W1AW          599\r\n"), "{f}");
        assert!(f.ends_with("END-OF-LOG:\r\n"));
        assert_eq!(modo_cabrillo("PSK31"), "DG");
        assert_eq!(modo_cabrillo("USB"), "PH");
    }
}
