//! Le regole del punteggio di un contest dei modi testuali, sezione `[Score]` del file `.contest` (`docs/contest-file.md`).
//!
//! Le regole sono pezzi riusabili:
//! - `points`: punti per relazione, la prima condizione vera vince (`same_dxcc=1; same_cont=2; other_cont=3`);
//! - `points_band`: fattore dei punti per banda (`80:2,40:2`);
//! - `bands`, `valid`: bande e condizione dei QSO ammessi (gli altri sono «non ammessi», con il motivo);
//! - `mult`: moltiplicatori `tipo/opzione/...` separati da `;` (tipi `dxcc`, `cq`, `itu`, `wpx`, `cont`, `area:K,VE`, `exch`;
//!   opzioni `band`, `mode`, `product`, `if:<condizione>`, `except:<paesi>`);
//! - `mult_band`: peso dei moltiplicatori per banda (`80:4,40:3`);
//! - `bonus`: punti in piu' per condizione (`call:RAEM=300`);
//! - `cqN`: tabella dei punti zona CQ -> zona CQ (riga della propria zona N, 40 valori), usata da `points=...=cqtab`;
//! - `special`: le regole scritte a mano (`raem`).
//!
//! Le condizioni sono pezzi uniti da `&`: `any`, `same_dxcc`, `same_cont`, `other_cont`, `dxcc:K,VE`, `not_dxcc:K,VE`, `cont:EU`,
//! `my_dxcc:..`, `my_cont:..`, `call:RAEM`, `band:80,40`, `mode:RTTY`.
//! Un file senza sezione `[Score]` usa le vecchie chiavi (`m_iPointsMyDxcc`, `m_iMultipliers`...), tradotte negli stessi pezzi.

use crate::elenco::{ContestTesto, Dominio};
use crate::geo;
use crate::punti_testo::{InfoPaese, QsoTesto, TipoMolt};
use std::collections::BTreeMap;

/// Un pezzo di condizione.
#[derive(Debug, Clone, PartialEq)]
pub enum Pezzo {
    Tutti,
    StessoPaese,
    StessoCont,
    AltroCont,
    Dxcc(Vec<String>),
    NonDxcc(Vec<String>),
    Cont(Vec<String>),
    MioDxcc(Vec<String>),
    MioCont(Vec<String>),
    Call(Vec<String>),
    Banda(Vec<String>),
    Modo(Vec<String>),
}

/// Una condizione: tutti i pezzi veri.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cond(pub Vec<Pezzo>);

/// Il valore dei punti di una regola: fisso o dalla tabella delle zone CQ.
#[derive(Debug, Clone, PartialEq)]
pub enum Valore {
    Fisso(i64),
    TabellaCq,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RegolaPunti {
    pub cond: Cond,
    pub valore: Valore,
}

/// Il tipo di un moltiplicatore.
#[derive(Debug, Clone, PartialEq)]
pub enum TipoM {
    Dxcc,
    Cq,
    Itu,
    Wpx,
    Cont,
    /// Le aree numeriche dei paesi elencati (`K1`, `VE3`, `JA7`).
    Area(Vec<String>),
    /// Lo scambio ricevuto (stato, provincia, oblast): dall'elenco dei domini, o l'ultima parola non numerica.
    Scambio,
}

impl TipoM {
    pub fn nome(&self) -> &'static str {
        match self {
            TipoM::Dxcc => "dxcc",
            TipoM::Cq => "cq",
            TipoM::Itu => "itu",
            TipoM::Wpx => "wpx",
            TipoM::Cont => "cont",
            TipoM::Area(_) => "area",
            TipoM::Scambio => "dom",
        }
    }
    pub fn tipo(&self) -> TipoMolt {
        match self {
            TipoM::Dxcc => TipoMolt::Dxcc,
            TipoM::Cq => TipoMolt::Cq,
            TipoM::Itu => TipoMolt::Itu,
            TipoM::Wpx => TipoMolt::Wpx,
            TipoM::Cont => TipoMolt::Cont,
            TipoM::Area(_) => TipoMolt::Area,
            TipoM::Scambio => TipoMolt::Dominio,
        }
    }
}

/// Un moltiplicatore del contest.
#[derive(Debug, Clone, PartialEq)]
pub struct DefMolt {
    pub tipo: TipoM,
    pub per_banda: bool,
    pub per_modo: bool,
    /// Moltiplica a parte (i continenti del BARTG): punteggio = punti x somma degli altri x questi.
    pub prodotto: bool,
    pub se: Option<Cond>,
    pub eccetto: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RegoleScore {
    pub punti: Vec<RegolaPunti>,
    pub fattore_banda: BTreeMap<String, i64>,
    pub bande: Vec<String>,
    pub validi: Option<Cond>,
    pub molt: Vec<DefMolt>,
    pub peso_banda: BTreeMap<String, i64>,
    pub bonus: Vec<(Cond, i64)>,
    /// `tabella_cq[mia-1][sua-1]`.
    pub tabella_cq: Vec<Vec<i64>>,
    pub speciale: String,
}

/// Una banda senza la `m` (`20m` -> `20`).
pub fn banda_corta(b: &str) -> String {
    b.trim().trim_end_matches(['m', 'M']).to_string()
}

fn lista(s: &str) -> Vec<String> {
    s.split(',').map(|x| x.trim().to_ascii_uppercase()).filter(|x| !x.is_empty()).collect()
}

fn mappa(s: &str) -> BTreeMap<String, i64> {
    s.split(',')
        .filter_map(|x| {
            let (b, v) = x.split_once(':')?;
            Some((banda_corta(b), v.trim().parse().ok()?))
        })
        .collect()
}

/// Legge una condizione (`same_cont&dxcc:K,VE`).
pub fn leggi_cond(s: &str) -> Result<Cond, String> {
    let mut v = Vec::new();
    for p in s.split('&').map(str::trim).filter(|p| !p.is_empty()) {
        let (k, arg) = p.split_once(':').map_or((p, ""), |(a, b)| (a.trim(), b));
        let l = || lista(arg);
        v.push(match k.to_ascii_lowercase().as_str() {
            "any" => Pezzo::Tutti,
            "same_dxcc" => Pezzo::StessoPaese,
            "same_cont" => Pezzo::StessoCont,
            "other_cont" => Pezzo::AltroCont,
            "dxcc" => Pezzo::Dxcc(l()),
            "not_dxcc" => Pezzo::NonDxcc(l()),
            "cont" => Pezzo::Cont(l()),
            "my_dxcc" => Pezzo::MioDxcc(l()),
            "my_cont" => Pezzo::MioCont(l()),
            "call" => Pezzo::Call(l()),
            "band" => Pezzo::Banda(arg.split(',').map(banda_corta).collect()),
            "mode" => Pezzo::Modo(l()),
            x => return Err(format!("condizione sconosciuta: {x}")),
        });
    }
    if v.is_empty() {
        return Err("condizione vuota".into());
    }
    Ok(Cond(v))
}

fn leggi_molt(s: &str) -> Result<DefMolt, String> {
    let mut parti = s.split('/').map(str::trim);
    let t = parti.next().unwrap_or_default();
    let (k, arg) = t.split_once(':').map_or((t, ""), |(a, b)| (a.trim(), b));
    let tipo = match k.to_ascii_lowercase().as_str() {
        "dxcc" => TipoM::Dxcc,
        "cq" => TipoM::Cq,
        "itu" => TipoM::Itu,
        "wpx" => TipoM::Wpx,
        "cont" => TipoM::Cont,
        "area" => TipoM::Area(lista(arg)),
        "exch" => TipoM::Scambio,
        x => return Err(format!("moltiplicatore sconosciuto: {x}")),
    };
    let mut d = DefMolt { tipo, per_banda: false, per_modo: false, prodotto: false, se: None, eccetto: Vec::new() };
    for o in parti.filter(|o| !o.is_empty()) {
        let (k, arg) = o.split_once(':').map_or((o, ""), |(a, b)| (a.trim(), b));
        match k.to_ascii_lowercase().as_str() {
            "band" => d.per_banda = true,
            "mode" => d.per_modo = true,
            "once" => {}
            "product" => d.prodotto = true,
            "if" => d.se = Some(leggi_cond(arg)?),
            "except" => d.eccetto = lista(arg),
            x => return Err(format!("opzione sconosciuta: {x}")),
        }
    }
    Ok(d)
}

impl RegoleScore {
    /// Legge le righe `chiave=valore` della sezione `[Score]`.
    pub fn da_testo(testo: &str) -> Result<RegoleScore, String> {
        let mut r = RegoleScore::default();
        let mut righe_cq: BTreeMap<usize, Vec<i64>> = BTreeMap::new();
        for riga in testo.lines().map(str::trim).filter(|x| !x.is_empty() && !x.starts_with(';')) {
            let Some((k, v)) = riga.split_once('=') else { continue };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            match k.as_str() {
                "points" => {
                    for p in v.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                        let (c, n) = p.rsplit_once('=').ok_or_else(|| format!("regola dei punti senza valore: {p}"))?;
                        let valore = if n.trim().eq_ignore_ascii_case("cqtab") { Valore::TabellaCq } else { Valore::Fisso(n.trim().parse().map_err(|_| format!("punti non validi: {n}"))?) };
                        r.punti.push(RegolaPunti { cond: leggi_cond(c)?, valore });
                    }
                }
                "points_band" => r.fattore_banda = mappa(v),
                "bands" => r.bande = v.split(',').map(banda_corta).filter(|b| !b.is_empty()).collect(),
                "valid" => r.validi = if v.is_empty() { None } else { Some(leggi_cond(v)?) },
                "mult" => {
                    for m in v.split(';').map(str::trim).filter(|m| !m.is_empty()) {
                        r.molt.push(leggi_molt(m)?);
                    }
                }
                "mult_band" => r.peso_banda = mappa(v),
                "bonus" => {
                    for p in v.split(';').map(str::trim).filter(|p| !p.is_empty()) {
                        let (c, n) = p.rsplit_once('=').ok_or_else(|| format!("bonus senza valore: {p}"))?;
                        r.bonus.push((leggi_cond(c)?, n.trim().parse().map_err(|_| format!("bonus non valido: {n}"))?));
                    }
                }
                "special" => r.speciale = v.to_ascii_lowercase(),
                _ => {
                    if let Some(n) = k.strip_prefix("cq").and_then(|n| n.parse::<usize>().ok()).filter(|n| (1..=40).contains(n)) {
                        righe_cq.insert(n, v.split(',').map(|x| x.trim().parse().unwrap_or(0)).collect());
                    }
                }
            }
        }
        if !righe_cq.is_empty() {
            r.tabella_cq = (1..=40).map(|n| righe_cq.remove(&n).unwrap_or_default()).collect();
        }
        Ok(r)
    }

    /// Le regole equivalenti alle vecchie chiavi del file (senza sezione `[Score]`).
    pub fn dalle_chiavi(c: &ContestTesto) -> RegoleScore {
        let mut r = RegoleScore::default();
        if c.points_my_dxcc == 0 && c.points_my_cont == 0 && c.points_dx == 0 {
            r.punti.push(RegolaPunti { cond: Cond(vec![Pezzo::Tutti]), valore: Valore::Fisso(1) });
        } else {
            r.punti.push(RegolaPunti { cond: Cond(vec![Pezzo::StessoPaese]), valore: Valore::Fisso(i64::from(c.points_my_dxcc)) });
            r.punti.push(RegolaPunti { cond: Cond(vec![Pezzo::StessoCont]), valore: Valore::Fisso(i64::from(c.points_my_cont)) });
            r.punti.push(RegolaPunti { cond: Cond(vec![Pezzo::Tutti]), valore: Valore::Fisso(i64::from(c.points_dx)) });
        }
        for t in crate::punti_testo::tipi_molt(c) {
            let tipo = match t {
                TipoMolt::Dxcc => TipoM::Dxcc,
                TipoMolt::Wpx => TipoM::Wpx,
                TipoMolt::Cq => TipoM::Cq,
                TipoMolt::Itu => TipoM::Itu,
                _ => TipoM::Scambio,
            };
            r.molt.push(DefMolt { tipo, per_banda: c.band_mult, per_modo: false, prodotto: false, se: None, eccetto: Vec::new() });
        }
        for b in &c.bands {
            if b.on && b.weight > 0 {
                r.peso_banda.insert(b.band.clone(), i64::from(b.weight));
            }
        }
        r
    }

    /// Le regole di un contest: la sezione `[Score]` se c'e', altrimenti le vecchie chiavi.
    pub fn del_contest(c: &ContestTesto) -> Result<RegoleScore, String> {
        if c.score.trim().is_empty() { Ok(RegoleScore::dalle_chiavi(c)) } else { RegoleScore::da_testo(&c.score) }
    }
}

/// Il QSO con tutto quello che serve per le regole.
pub struct Ctx<'a> {
    pub q: &'a QsoTesto,
    pub call: &'a str,
    pub info: Option<&'a InfoPaese>,
    pub mio: Option<&'a InfoPaese>,
}

fn in_lista(l: &[String], v: &str) -> bool {
    !v.is_empty() && l.iter().any(|x| x.eq_ignore_ascii_case(v))
}

impl Pezzo {
    fn vero(&self, x: &Ctx) -> bool {
        let dx = x.info.map_or("", |i| i.dxcc.as_str());
        let ct = x.info.map_or("", |i| i.continente.as_str());
        let mdx = x.mio.map_or("", |i| i.dxcc.as_str());
        let mct = x.mio.map_or("", |i| i.continente.as_str());
        match self {
            Pezzo::Tutti => true,
            Pezzo::StessoPaese => !dx.is_empty() && dx == mdx,
            Pezzo::StessoCont => !ct.is_empty() && ct == mct,
            Pezzo::AltroCont => ct.is_empty() || mct.is_empty() || ct != mct,
            Pezzo::Dxcc(l) => in_lista(l, dx),
            Pezzo::NonDxcc(l) => !in_lista(l, dx),
            Pezzo::Cont(l) => in_lista(l, ct),
            Pezzo::MioDxcc(l) => in_lista(l, mdx),
            Pezzo::MioCont(l) => in_lista(l, mct),
            Pezzo::Call(l) => l.iter().any(|c| c.eq_ignore_ascii_case(x.call) || x.call.split('/').any(|p| p.eq_ignore_ascii_case(c))),
            Pezzo::Banda(l) => l.contains(&banda_corta(&x.q.band)),
            Pezzo::Modo(l) => in_lista(l, x.q.mode.trim()),
        }
    }
}

impl Cond {
    pub fn vera(&self, x: &Ctx) -> bool {
        self.0.iter().all(|p| p.vero(x))
    }
}

/// Il QSO e' ammesso? `Err(motivo)`: `band` (banda fuori dal contest) o `valid` (paese o stazione non ammessi).
pub fn ammesso(r: &RegoleScore, x: &Ctx) -> Result<(), &'static str> {
    if !r.bande.is_empty() && !r.bande.contains(&banda_corta(&x.q.band)) {
        return Err("band");
    }
    if let Some(c) = &r.validi
        && !c.vera(x)
    {
        return Err("valid");
    }
    Ok(())
}

/// I punti di un QSO ammesso: la prima regola vera per il fattore della banda, piu' i bonus e le regole scritte a mano.
pub fn punti(r: &RegoleScore, x: &Ctx) -> i64 {
    let base = r
        .punti
        .iter()
        .find(|p| p.cond.vera(x))
        .map_or(0, |p| match p.valore {
            Valore::Fisso(n) => n,
            Valore::TabellaCq => {
                let (m, s) = (x.mio.map_or(0, |i| usize::from(i.cq)), x.info.map_or(0, |i| usize::from(i.cq)));
                if m == 0 || s == 0 { 0 } else { r.tabella_cq.get(m - 1).and_then(|row| row.get(s - 1)).copied().unwrap_or(0) }
            }
        });
    let fattore = r.fattore_banda.get(&banda_corta(&x.q.band)).copied().unwrap_or(1);
    let bonus: i64 = r.bonus.iter().filter(|(c, _)| c.vera(x)).map(|(_, n)| n).sum();
    let speciale = if r.speciale == "raem" { punti_raem(x.q) } else { 0 };
    base * fattore + bonus + speciale
}

/// Le coordinate di uno scambio RAEM (`001 57N85E`, `57N 85O`): latitudine e longitudine in gradi interi con il segno.
pub fn coordinate_raem(scambio: &str) -> Option<(i64, i64)> {
    // le parole di sole cifre (numero, rapporto) non sono coordinate
    let s: String = scambio.to_ascii_uppercase().split_whitespace().filter(|p| !p.bytes().all(|b| b.is_ascii_digit())).collect::<Vec<_>>().join("");
    // la prima cifra seguita da N o S, poi cifre seguite da E, O (ost) o W
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let inizio = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i > inizio && i < b.len() && (b[i] == b'N' || b[i] == b'S') {
            let lat: i64 = s[inizio..i].parse().ok()?;
            let lat = if b[i] == b'S' { -lat } else { lat };
            let j0 = i + 1;
            let mut j = j0;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > j0 && j < b.len() && matches!(b[j], b'E' | b'O' | b'W') {
                let lon: i64 = s[j0..j].parse().ok()?;
                return Some((lat, if b[j] == b'W' { -lon } else { lon }));
            }
            return None;
        }
        i = inizio + 1;
    }
    None
}

/// RAEM (scritto a mano): un punto per grado di differenza di latitudine e di longitudine fra le coordinate mandate e ricevute,
/// piu' 100 se la stazione lavorata e' polare (latitudine 66 o piu'). I 50 punti del QSO e i 300 della stazione RAEM sono nelle regole.
pub fn punti_raem(q: &QsoTesto) -> i64 {
    let (Some(mie), Some(sue)) = (coordinate_raem(&q.exch_sent), coordinate_raem(&q.exch_rcvd)) else { return 0 };
    let mut dlon = (mie.1 - sue.1).abs();
    if dlon > 180 {
        dlon = 360 - dlon;
    }
    (mie.0 - sue.0).abs() + dlon + if sue.0.abs() >= 66 { 100 } else { 0 }
}

/// Il fattore finale in millesimi: RAEM x1,1 per chi sta dentro i circoli polari (dalle coordinate mandate), altrimenti 1.
pub fn fattore_finale(r: &RegoleScore, qsos: &[QsoTesto]) -> i64 {
    if r.speciale == "raem" && qsos.iter().find_map(|q| coordinate_raem(&q.exch_sent)).is_some_and(|(lat, _)| lat.abs() >= 66) { 1100 } else { 1000 }
}

/// L'area numerica di un nominativo: la cifra dopo la barra (`K5DJ/1`), altrimenti l'ultima cifra del prefisso WPX.
pub fn cifra_area(call: &str) -> Option<char> {
    let up = call.trim().to_ascii_uppercase();
    if let Some(d) = up.split('/').skip(1).find(|p| p.len() == 1 && p.bytes().all(|b| b.is_ascii_digit())) {
        return d.chars().next();
    }
    geo::prefisso_wpx(&up).chars().rev().find(char::is_ascii_digit)
}

/// Il valore di un moltiplicatore per un QSO (`None`: il QSO non lo porta).
pub fn valore_molt(d: &DefMolt, x: &Ctx, dominio: Option<&Dominio>) -> Option<String> {
    if d.se.as_ref().is_some_and(|c| !c.vera(x)) {
        return None;
    }
    let dx = x.info.map_or("", |i| i.dxcc.as_str());
    if in_lista(&d.eccetto, dx) {
        return None;
    }
    let v = match &d.tipo {
        TipoM::Dxcc => dx.to_string(),
        TipoM::Cq => x.info.filter(|i| i.cq > 0).map(|i| i.cq.to_string()).unwrap_or_default(),
        TipoM::Itu => x.info.filter(|i| i.itu > 0).map(|i| i.itu.to_string()).unwrap_or_default(),
        TipoM::Wpx => geo::prefisso_wpx(x.call),
        TipoM::Cont => x.info.map(|i| i.continente.clone()).unwrap_or_default(),
        TipoM::Area(l) => {
            if !in_lista(l, dx) {
                return None;
            }
            cifra_area(x.call).map(|c| format!("{dx}{c}")).unwrap_or_default()
        }
        TipoM::Scambio => {
            let parole: Vec<&str> = x.q.exch_rcvd.split_whitespace().collect();
            match dominio {
                Some(dm) => parole.iter().find_map(|p| dm.cerca(p, x.call)).unwrap_or_default(),
                None => parole.iter().rev().find(|p| !p.bytes().all(|b| b.is_ascii_digit())).or(parole.last()).map(|p| p.to_ascii_uppercase()).unwrap_or_default(),
            }
        }
    };
    let v = v.trim().to_ascii_uppercase();
    (!v.is_empty()).then_some(v)
}

/// Il peso di un moltiplicatore per banda (1 se la banda non ha peso).
pub fn peso(r: &RegoleScore, banda: &str) -> i64 {
    r.peso_banda.get(&banda_corta(banda)).copied().filter(|p| *p > 0).unwrap_or(1)
}

/// I valori possibili di un tipo con un elenco chiuso (per i moltiplicatori che mancano): zone, continenti, domini.
pub fn valori_possibili(t: &TipoM, dominio: Option<&Dominio>) -> Vec<String> {
    match t {
        TipoM::Cq => (1..=40).map(|n: u8| n.to_string()).collect(),
        TipoM::Itu => (1..=90).map(|n: u8| n.to_string()).collect(),
        TipoM::Cont => ["AF", "AS", "EU", "NA", "OC", "SA"].iter().map(|s| (*s).to_string()).collect(),
        TipoM::Area(l) => l.iter().flat_map(|p| (0..=9).map(move |n| format!("{p}{n}"))).collect(),
        TipoM::Scambio => {
            let mut v: Vec<String> = dominio.map(|d| d.entries.iter().map(|e| e.value.trim().to_ascii_uppercase()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
            v.sort();
            v.dedup();
            v
        }
        TipoM::Dxcc | TipoM::Wpx => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legge_le_regole() {
        let r = RegoleScore::da_testo("points=same_dxcc=1; same_cont=2; other_cont=3\npoints_band=80:2,40:2\nmult=cq/band; dxcc/band/except:K; exch/if:dxcc:K,VE/once; cont/product\nmult_band=80m:4\nbonus=call:RAEM=300\ncq1=1,2,3").unwrap_or_default();
        assert_eq!(r.punti.len(), 3);
        assert_eq!(r.fattore_banda.get("80"), Some(&2));
        assert_eq!(r.molt.len(), 4);
        assert!(r.molt[0].per_banda && r.molt[3].prodotto);
        assert_eq!(r.molt[1].eccetto, vec!["K".to_string()]);
        assert!(r.molt[2].se.is_some());
        assert_eq!(peso(&r, "80m"), 4);
        assert_eq!(r.tabella_cq[0], vec![1, 2, 3]);
        assert!(RegoleScore::da_testo("mult=pippo").is_err());
        assert!(RegoleScore::da_testo("points=boh=1").is_err());
    }

    #[test]
    fn coordinate_e_aree() {
        assert_eq!(coordinate_raem("001 57N85E"), Some((57, 85)));
        assert_eq!(coordinate_raem("012 45N 12O"), Some((45, 12)));
        assert_eq!(coordinate_raem("599 40N74W"), Some((40, -74)));
        assert_eq!(coordinate_raem("599 001"), None);
        assert_eq!(cifra_area("K5DJ/1"), Some('1'));
        assert_eq!(cifra_area("JA7XYZ"), Some('7'));
    }
}
