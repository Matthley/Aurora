//! Aiuti del banco RTTY a livello di carattere (`tests/rtty_caratteri.rs`, `docs/banco-rtty.md`): generatore Baudot ITA2 con i tempi di
//! fine di ogni carattere, testo da contest con seme fisso, canale (rumore, evanescenza, deriva, stazione vicina) e allineamento di
//! Levenshtein con le operazioni.

#![allow(dead_code)]

use std::f64::consts::PI;

pub const RATE: f64 = 12_000.0;
pub const BAUD: f64 = 45.45;
pub const SHIFT: f64 = 170.0;
pub const STOP_BITS: f64 = 1.5;
/// Ampiezza di picco di ciascun tono del segnale voluto.
pub const AMP: f64 = 0.3;

/// ITA2 come lo legge il ricevitore (indice = codice a 5 bit). 0 = nessun carattere.
pub const LETTERE: [u8; 32] = [
    0, b'E', b'\n', b'A', b' ', b'S', b'I', b'U', b'\r', b'D', b'R', b'J', b'N', b'F', b'C', b'K', b'T', b'Z', b'L', b'W', b'H', b'Y', b'P',
    b'Q', b'O', b'B', b'G', 0, b'M', b'X', b'V', 0,
];
pub const CIFRE: [u8; 32] = [
    0, b'3', b'\n', b'-', b' ', 0, b'8', b'7', b'\r', b'$', b'4', b'\'', b',', b'!', b':', b'(', b'5', b'"', b')', b'2', b'#', b'6', b'0',
    b'1', b'9', b'?', b'&', 0, b'.', b'/', b';', 0,
];
pub const FIGS: u8 = 27;
pub const LTRS: u8 = 31;
pub const SPAZIO: u8 = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Reg {
    Lettere,
    Cifre,
    /// Dopo uno spazio fra le cifre: chi riceve con l'UOS e' tornato alle lettere, chi no e' rimasto nelle cifre.
    Ignoto,
}

/// Codice e registro richiesto (`None` = vale in tutti e due: spazio, CR, LF).
pub fn codifica(c: char) -> Option<(u8, Option<bool>)> {
    let u = c.to_ascii_uppercase();
    if !u.is_ascii() {
        return None;
    }
    let b = u as u8;
    let l = LETTERE.iter().position(|&x| x == b && x != 0);
    let f = CIFRE.iter().position(|&x| x == b && x != 0);
    match (l, f) {
        (Some(i), Some(_)) => Some((i as u8, None)),
        (Some(i), None) => Some((i as u8, Some(false))),
        (None, Some(i)) => Some((i as u8, Some(true))),
        _ => None,
    }
}

/// Una trasmissione: secondi di silenzio (portante spenta) prima, e il testo.
#[derive(Clone, Debug)]
pub struct Passaggio {
    pub silenzio_prima_s: f64,
    pub testo: String,
}

/// Il segnale a toni ideali: per ogni tratto mark/space la durata in secondi (`None` = portante spenta).
#[derive(Clone, Debug, Default)]
pub struct Trama {
    pub tratti: Vec<(Option<bool>, f64)>,
    /// Ogni carattere stampabile mandato con l'istante (s) in cui finisce il suo stop.
    pub caratteri: Vec<(char, f64)>,
    /// Intervalli (s) a portante spenta.
    pub silenzi: Vec<(f64, f64)>,
    /// Ogni trasmissione: accensione della portante e fine (s).
    pub passaggi: Vec<(f64, f64)>,
    pub durata_s: f64,
}

impl Trama {
    fn push(&mut self, stato: Option<bool>, s: f64) {
        self.tratti.push((stato, s));
        self.durata_s += s;
    }

    fn quadro(&mut self, code: u8) {
        let t = 1.0 / BAUD;
        self.push(Some(false), t);
        for b in 0..5 {
            self.push(Some((code >> b) & 1 == 1), t);
        }
        self.push(Some(true), STOP_BITS * t);
    }
}

/// I passaggi in bit: per ognuno 0,3 s di mark, LTRS, CR LF, il testo, CR LF e 0,2 s di mark. Il cambio di registro si manda solo quando
/// serve; dopo uno spazio fra le cifre il registro di chi riceve non e' noto e il carattere seguente si fa precedere dal suo FIGS/LTRS.
pub fn trama(passaggi: &[Passaggio]) -> Trama {
    let mut tr = Trama::default();
    for p in passaggi {
        if p.silenzio_prima_s > 0.0 {
            let a = tr.durata_s;
            tr.push(None, p.silenzio_prima_s);
            tr.silenzi.push((a, tr.durata_s));
        }
        let inizio = tr.durata_s;
        tr.push(Some(true), 0.3);
        tr.quadro(LTRS);
        let mut reg = Reg::Lettere;
        let testo = format!("\r\n{}\r\n", p.testo);
        for c in testo.chars() {
            let Some((code, serve)) = codifica(c) else { continue };
            match serve {
                None => {
                    tr.quadro(code);
                    if code == SPAZIO && reg == Reg::Cifre {
                        reg = Reg::Ignoto;
                    }
                }
                Some(cifre) => {
                    let voluto = if cifre { Reg::Cifre } else { Reg::Lettere };
                    if reg != voluto {
                        tr.quadro(if cifre { FIGS } else { LTRS });
                        reg = voluto;
                    }
                    tr.quadro(code);
                }
            }
            tr.caratteri.push((c.to_ascii_uppercase(), tr.durata_s));
        }
        tr.push(Some(true), 0.2);
        tr.passaggi.push((inizio, tr.durata_s));
    }
    tr
}

/// Decodifica ideale dei tratti (per la prova del generatore): rilegge i quadri e restituisce il testo, con UOS.
pub fn rileggi(tr: &Trama) -> String {
    let t = 1.0 / BAUD;
    // campiona i tratti al centro dei bit
    let mut bit: Vec<bool> = Vec::new();
    let mut out = String::new();
    let mut cifre = false;
    let mut iter = tr.tratti.iter().peekable();
    while let Some(&(stato, d)) = iter.next() {
        if stato != Some(false) || (d - t).abs() > 1e-9 {
            continue;
        }
        bit.clear();
        for _ in 0..5 {
            if let Some(&&(s, _)) = iter.peek() {
                bit.push(s == Some(true));
                iter.next();
            }
        }
        iter.next(); // stop
        let code = bit.iter().enumerate().fold(0u8, |a, (i, &b)| a | (u8::from(b) << i));
        match code {
            FIGS => cifre = true,
            LTRS => cifre = false,
            _ => {
                let c = if cifre { CIFRE[usize::from(code)] } else { LETTERE[usize::from(code)] };
                if c != 0 {
                    out.push(c as char);
                }
                if code == SPAZIO {
                    cifre = false;
                }
            }
        }
    }
    out
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seme: u64) -> Self {
        Rng(seme.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn unif(&mut self) -> f64 {
        (self.u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn idx(&mut self, n: usize) -> usize {
        (self.u64() % n as u64) as usize
    }
    pub fn gauss(&mut self) -> f64 {
        let u1 = self.unif().max(1e-300);
        let u2 = self.unif();
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

/// Nominativi di prova (inventati).
pub const NOMINATIVI: [&str; 12] =
    ["IU3ABC", "DL1XYZ", "F5QRT", "EA3KLM", "OK1FPX", "SP9ABJ", "G4ZZT", "PA3DXQ", "OH2KWB", "S57RTY", "IZ2QWE", "HB9TST"];

/// Passaggi da contest fino a `durata_s` secondi d'aria, con `silenzio_s` di portante spenta fra uno e l'altro.
pub fn testo_contest(seme: u64, durata_s: f64, silenzio_s: f64) -> Vec<Passaggio> {
    testo_contest_silenzi(seme, durata_s, silenzio_s, silenzio_s)
}

/// Come [`testo_contest`], con silenzi a caso fra `min_s` e `max_s`.
pub fn testo_contest_silenzi(seme: u64, durata_s: f64, min_s: f64, max_s: f64) -> Vec<Passaggio> {
    let mut r = Rng::new(seme);
    let mut out = Vec::new();
    let mut aria = 0.0;
    let mut nr = 7 + r.idx(40);
    while aria < durata_s {
        let a = NOMINATIVI[r.idx(NOMINATIVI.len())];
        let mut b = NOMINATIVI[r.idx(NOMINATIVI.len())];
        if b == a {
            b = NOMINATIVI[(r.idx(NOMINATIVI.len() - 1) + 1 + NOMINATIVI.iter().position(|x| *x == a).unwrap_or(0)) % NOMINATIVI.len()];
        }
        nr += 1 + r.idx(3);
        let testo = match r.idx(5) {
            0 => format!("CQ TEST {a} {a} TEST"),
            1 => format!("{b} {b}"),
            2 => format!("{b} 599 {nr:03} {nr:03} {b}"),
            3 => format!("TU {a} 599 {nr:03} {nr:03}"),
            _ => format!("TU {b} {a} TEST"),
        };
        aria += (testo.len() as f64 + 6.0) * 7.5 / BAUD + 0.5;
        let silenzio_s = min_s + (max_s - min_s) * r.unif();
        out.push(Passaggio { silenzio_prima_s: if out.is_empty() { 0.5 } else { silenzio_s }, testo });
    }
    out
}

/// Evanescenza di Rayleigh: guadagno complesso con potenza media 1, banda Doppler `fd` (Hz), filtro a un polo.
pub struct Rayleigh {
    a: f64,
    b: f64,
    x: f64,
    y: f64,
}

impl Rayleigh {
    pub fn new(fd: f64, r: &mut Rng) -> Self {
        let a = (-2.0 * PI * fd / RATE).exp();
        Rayleigh { a, b: (1.0 - a * a).sqrt(), x: r.gauss(), y: r.gauss() }
    }
    pub fn passo(&mut self, r: &mut Rng) -> f64 {
        self.x = self.a * self.x + self.b * r.gauss();
        self.y = self.a * self.y + self.b * r.gauss();
        ((self.x * self.x + self.y * self.y) / 2.0).sqrt()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Fading {
    Nessuno,
    /// Piatto: tutti e due i toni insieme (Doppler Hz).
    Piatto(f64),
    /// Selettivo: mark e space indipendenti (Doppler Hz).
    Selettivo(f64),
    /// Solo il mark svanisce, lo space resta fermo (Doppler Hz).
    UnTono(f64),
}

/// Audio del segnale voluto (12 kHz): toni a fase continua, `centro_hz` piu' una deriva lineare di `deriva_hz` lungo tutto il segnale,
/// con l'evanescenza data. Ogni tono ha ampiezza `amp` (in media).
pub fn sintetizza(tr: &Trama, centro_hz: f64, deriva_hz: f64, amp: f64, fading: Fading, r: &mut Rng) -> Vec<f32> {
    let n_tot = (tr.durata_s * RATE).ceil() as usize + 1;
    let mut out = Vec::with_capacity(n_tot);
    let (fd1, fd2, sel) = match fading {
        Fading::Nessuno => (0.0, 0.0, false),
        Fading::Piatto(fd) => (fd, 0.0, false),
        Fading::Selettivo(fd) => (fd, fd, true),
        Fading::UnTono(fd) => (fd, 0.0, true),
    };
    let mut g1 = (fd1 > 0.0).then(|| Rayleigh::new(fd1, r));
    let mut g2 = (fd2 > 0.0).then(|| Rayleigh::new(fd2, r));
    let mut fase = 0.0f64;
    let mut t_fine = 0.0;
    for &(stato, d) in &tr.tratti {
        t_fine += d;
        while (out.len() as f64) < t_fine * RATE {
            let t = out.len() as f64 / RATE;
            let gm = g1.as_mut().map_or(1.0, |g| g.passo(r));
            let gs = if sel { g2.as_mut().map_or(1.0, |g| g.passo(r)) } else { gm };
            let centro = centro_hz + deriva_hz * t / tr.durata_s.max(1.0);
            let s = match stato {
                None => 0.0,
                Some(mark) => {
                    let f = if mark { centro + SHIFT / 2.0 } else { centro - SHIFT / 2.0 };
                    fase = (fase + 2.0 * PI * f / RATE) % (2.0 * PI);
                    amp * if mark { gm } else { gs } * fase.sin()
                }
            };
            out.push(s as f32);
        }
    }
    out
}

/// Deviazione del rumore bianco (a 12 kHz) per un S/N in 2,5 kHz rispetto a un tono di ampiezza `amp` (potenza amp^2/2).
pub fn sigma_per_snr(snr_db: f64, amp: f64) -> f64 {
    let p_segnale = amp * amp / 2.0;
    let p_rumore_25k = p_segnale / 10f64.powf(snr_db / 10.0);
    (p_rumore_25k * (RATE / 2.0) / 2500.0).sqrt()
}

/// Crepitii: in media `per_s` scariche al secondo, ognuna lunga 5-50 ms, rumore con deviazione `sigma`.
pub fn aggiungi_impulsi(x: &mut [f32], per_s: f64, sigma: f64, r: &mut Rng) {
    let p = per_s / RATE;
    let mut i = 0;
    while i < x.len() {
        if r.unif() < p {
            let n = ((0.005 + 0.045 * r.unif()) * RATE) as usize;
            for v in x.iter_mut().skip(i).take(n) {
                *v += (sigma * r.gauss()) as f32;
            }
            i += n;
        }
        i += 1;
    }
}

pub fn aggiungi_rumore(x: &mut [f32], sigma: f64, r: &mut Rng) {
    for v in x.iter_mut() {
        *v += (sigma * r.gauss()) as f32;
    }
}

/// Testo per il confronto: maiuscolo, CR/LF come spazio, controlli tolti, spazi ripetuti in uno solo, senza spazi in testa e in coda.
/// Ogni carattere tiene il suo istante (per gli spazi, quello del primo).
pub fn normalizza(x: &[(char, f64)]) -> Vec<(char, f64)> {
    let mut out: Vec<(char, f64)> = Vec::with_capacity(x.len());
    for &(c, t) in x {
        let c = match c {
            '\r' | '\n' | '\t' => ' ',
            c if (c as u32) < 0x20 => continue,
            c => c.to_ascii_uppercase(),
        };
        if c == ' ' && out.last().is_none_or(|p| p.0 == ' ') {
            continue;
        }
        out.push((c, t));
    }
    while out.last().is_some_and(|p| p.0 == ' ') {
        out.pop();
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    /// Giusto: indice atteso, indice letto.
    Giusto(usize, usize),
    Sbagliato(usize, usize),
    /// Mancante: indice atteso.
    Mancante(usize),
    /// Inserito: indice letto.
    Inserito(usize),
}

/// Allineamento di Levenshtein (costo 1 per sostituzione, cancellazione, inserimento) con il percorso.
pub fn allinea(a: &[char], b: &[char]) -> Vec<Op> {
    let (n, m) = (a.len(), b.len());
    let w = m + 1;
    let mut d = vec![0u32; (n + 1) * w];
    for j in 0..=m {
        d[j] = j as u32;
    }
    for i in 1..=n {
        d[i * w] = i as u32;
        for j in 1..=m {
            let sost = d[(i - 1) * w + j - 1] + u32::from(a[i - 1] != b[j - 1]);
            d[i * w + j] = sost.min(d[(i - 1) * w + j] + 1).min(d[i * w + j - 1] + 1);
        }
    }
    let mut ops = Vec::with_capacity(n.max(m));
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        let qui = d[i * w + j];
        if i > 0 && j > 0 && qui == d[(i - 1) * w + j - 1] + u32::from(a[i - 1] != b[j - 1]) {
            ops.push(if a[i - 1] == b[j - 1] { Op::Giusto(i - 1, j - 1) } else { Op::Sbagliato(i - 1, j - 1) });
            i -= 1;
            j -= 1;
        } else if i > 0 && qui == d[(i - 1) * w + j] + 1 {
            ops.push(Op::Mancante(i - 1));
            i -= 1;
        } else {
            ops.push(Op::Inserito(j - 1));
            j -= 1;
        }
    }
    ops.reverse();
    ops
}

#[derive(Clone, Debug, Default)]
pub struct Misura {
    pub atteso: usize,
    pub giusti: usize,
    pub sbagliati: usize,
    pub mancanti: usize,
    pub inseriti: usize,
    pub call_tot: usize,
    pub call_esatti: usize,
    pub call_sbagliati: usize,
    /// Caratteri (non spazi) usciti nei silenzi, al minuto di silenzio (`None` se non ci sono silenzi lunghi).
    pub falsi_min: Option<f64>,
    /// Ritardi (ms) dei caratteri giusti; mediana e 95 %.
    pub lat_med_ms: Option<f64>,
    pub lat_p95_ms: Option<f64>,
    pub cpu_ms_s: f64,
}

impl Misura {
    pub fn cer(&self) -> f64 {
        if self.atteso == 0 {
            return 0.0;
        }
        100.0 * (self.sbagliati + self.mancanti + self.inseriti) as f64 / self.atteso as f64
    }
}

/// Confronta l'atteso (con i tempi di fine) con il letto (con i tempi di uscita). `silenzi`: intervalli senza segnale dove ogni carattere
/// (dopo 1 s di margine per il ritardo) e' falso.
pub fn misura(atteso: &[(char, f64)], letto: &[(char, f64)], silenzi: &[(f64, f64)], con_tempi: bool) -> Misura {
    let at = normalizza(atteso);
    let le = normalizza(letto);
    let a: Vec<char> = at.iter().map(|p| p.0).collect();
    let b: Vec<char> = le.iter().map(|p| p.0).collect();
    let ops = allinea(&a, &b);
    let mut m = Misura { atteso: a.len(), ..Misura::default() };
    let mut mappa: Vec<Option<usize>> = vec![None; a.len()];
    let mut lat = Vec::new();
    for op in &ops {
        match *op {
            Op::Giusto(i, j) => {
                m.giusti += 1;
                mappa[i] = Some(j);
                if con_tempi {
                    lat.push((le[j].1 - at[i].1) * 1000.0);
                }
            }
            Op::Sbagliato(i, j) => {
                m.sbagliati += 1;
                mappa[i] = Some(j);
            }
            Op::Mancante(_) => m.mancanti += 1,
            Op::Inserito(_) => m.inseriti += 1,
        }
    }
    // nominativi: le parole attese che sono nell'elenco; il letto e' il tratto fra il primo e l'ultimo carattere allineato
    let mut i = 0;
    while i < a.len() {
        if a[i] == ' ' {
            i += 1;
            continue;
        }
        let s = i;
        while i < a.len() && a[i] != ' ' {
            i += 1;
        }
        let parola: String = a[s..i].iter().collect();
        if !NOMINATIVI.contains(&parola.as_str()) {
            continue;
        }
        m.call_tot += 1;
        let idx: Vec<usize> = mappa[s..i].iter().flatten().copied().collect();
        if let (Some(&lo), Some(&hi)) = (idx.iter().min(), idx.iter().max()) {
            let letta: String = b[lo..=hi].iter().collect();
            if letta == parola {
                m.call_esatti += 1;
            } else {
                m.call_sbagliati += 1;
            }
        }
    }
    let minuti: f64 = silenzi.iter().map(|(x, y)| (y - x - 1.0).max(0.0)).sum::<f64>() / 60.0;
    if minuti > 0.0 {
        let falsi = letto.iter().filter(|(c, t)| c.is_ascii_graphic() && silenzi.iter().any(|(x, y)| *t >= x + 1.0 && *t <= *y)).count();
        m.falsi_min = Some(falsi as f64 / minuti);
    }
    if !lat.is_empty() {
        lat.sort_by(f64::total_cmp);
        m.lat_med_ms = Some(lat[lat.len() / 2]);
        m.lat_p95_ms = Some(lat[(lat.len() * 95 / 100).min(lat.len() - 1)]);
    }
    m
}

/// Una riga della tabella.
pub fn riga(scenario: &str, dec: &str, m: &Misura) -> String {
    let f = |v: Option<f64>, p: usize| v.map_or("-".to_string(), |x| format!("{x:.p$}"));
    format!(
        "{:<22} {:<7} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7.1} {:>3}/{:<3}/{:>3} {:>8} {:>7} {:>7} {:>7.2}",
        scenario,
        dec,
        m.atteso,
        m.giusti,
        m.sbagliati,
        m.mancanti,
        m.inseriti,
        m.cer(),
        m.call_esatti,
        m.call_sbagliati,
        m.call_tot,
        f(m.falsi_min, 1),
        f(m.lat_med_ms, 0),
        f(m.lat_p95_ms, 0),
        m.cpu_ms_s
    )
}

pub fn intestazione() -> String {
    format!(
        "{:<22} {:<7} {:>5} {:>5} {:>5} {:>5} {:>5} {:>7} {:>11} {:>8} {:>7} {:>7} {:>7}",
        "scenario", "dec", "att", "ok", "sbag", "manc", "ins", "CER%", "call ok/x/n", "falsi/m", "lat50", "lat95", "cpu"
    )
}

/// Misure del passaggio trasmissione -> silenzio -> trasmissione (taratura dello squelch).
#[derive(Clone, Debug, Default)]
pub struct MisuraSilenzi {
    /// Caratteri inseriti nel primo secondo dopo lo spegnimento della portante, al minuto di silenzio.
    pub coda_min: f64,
    /// Caratteri inseriti nel resto del silenzio, al minuto di silenzio.
    pub silenzio_min: f64,
    /// Dei primi 5 caratteri (non spazi) di ogni trasmissione, quanti non escono giusti (%).
    pub persi_inizio_pct: f64,
    /// Dall'accensione della portante al primo carattere giusto (ms): mediana e peggiore.
    pub primo_med_ms: Option<f64>,
    pub primo_max_ms: Option<f64>,
    /// Trasmissioni senza nessun carattere giusto.
    pub mute: usize,
}

pub fn misura_silenzi(tr: &Trama, letto: &[(char, f64)]) -> MisuraSilenzi {
    let at = normalizza(&tr.caratteri);
    let le = normalizza(letto);
    let a: Vec<char> = at.iter().map(|p| p.0).collect();
    let b: Vec<char> = le.iter().map(|p| p.0).collect();
    let ops = allinea(&a, &b);
    let mut giusto: Vec<Option<f64>> = vec![None; a.len()];
    let (mut coda, mut sil) = (0usize, 0usize);
    for op in &ops {
        match *op {
            Op::Giusto(i, j) => giusto[i] = Some(le[j].1),
            Op::Inserito(j) if b[j] != ' ' => {
                let t = le[j].1;
                for &(x, y) in &tr.silenzi {
                    if t >= x && t < (x + 1.0).min(y) {
                        coda += 1;
                    } else if t >= x + 1.0 && t <= y {
                        sil += 1;
                    }
                }
            }
            _ => {}
        }
    }
    let minuti: f64 = tr.silenzi.iter().map(|(x, y)| y - x).sum::<f64>() / 60.0;
    let mut m = MisuraSilenzi::default();
    if minuti > 0.0 {
        m.coda_min = coda as f64 / minuti;
        m.silenzio_min = sil as f64 / minuti;
    }
    let (mut persi, mut tot) = (0usize, 0usize);
    let mut primi = Vec::new();
    for &(s, e) in &tr.passaggi {
        let idx: Vec<usize> = (0..a.len()).filter(|&i| at[i].1 > s && at[i].1 <= e && a[i] != ' ').collect();
        for &i in idx.iter().take(5) {
            tot += 1;
            persi += usize::from(giusto[i].is_none());
        }
        match idx.iter().filter_map(|&i| giusto[i]).reduce(f64::min) {
            Some(t) => primi.push((t - s) * 1000.0),
            None => m.mute += 1,
        }
    }
    if tot > 0 {
        m.persi_inizio_pct = 100.0 * persi as f64 / tot as f64;
    }
    if !primi.is_empty() {
        primi.sort_by(f64::total_cmp);
        m.primo_med_ms = Some(primi[primi.len() / 2]);
        m.primo_max_ms = primi.last().copied();
    }
    m
}

pub fn intestazione_silenzi() -> String {
    format!(
        "{:<22} {:<7} {:>4} {:>7} {:>9} {:>8} {:>7} {:>7} {:>10}",
        "scenario", "dec", "tx", "CER%", "coda/min", "sil/min", "persi%", "primo50", "max/mute"
    )
}

pub fn riga_silenzi(scenario: &str, dec: &str, n_tx: usize, cer: f64, m: &MisuraSilenzi) -> String {
    let f = |v: Option<f64>| v.map_or("-".to_string(), |x| format!("{x:.0}"));
    format!(
        "{:<22} {:<7} {:>4} {:>7.1} {:>9.1} {:>8.1} {:>7.1} {:>7} {:>7}/{}",
        scenario, dec, n_tx, cer, m.coda_min, m.silenzio_min, m.persi_inizio_pct, f(m.primo_med_ms), f(m.primo_max_ms), m.mute
    )
}
