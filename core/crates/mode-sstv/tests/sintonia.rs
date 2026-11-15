//! Sintonia che cambia durante la ricezione: VFO girato a meta' immagine, deriva lenta, scarto iniziale senza VIS.
//!
//! Lo spostamento si applica all'audio gia' codificato con un traslatore di frequenza (segnale analitico per filtro di Hilbert
//! moltiplicato per una fase che avanza alla velocita' dello spostamento), come farebbe la radio girando la manopola.

use mode_sstv::{Decoder, DecoderConfig, DecoderEvent, RATE, RgbImage, SstvMode, encoder::encode_shifted, modes::HEADER_SECONDS};

fn test_image(w: u32, h: u32) -> RgbImage {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (f64::from(x) / f64::from(w), f64::from(y) / f64::from(h));
            let r = 40.0 + 180.0 * fx;
            let g = 60.0 + 150.0 * fy + 25.0 * (fx * 9.0).sin();
            let b = 200.0 - 140.0 * fx * fy;
            data.extend([r, g, b].map(|v: f64| v.clamp(0.0, 255.0) as u8));
        }
    }
    RgbImage { width: w, height: h, data }
}

struct Noise(u64);

impl Noise {
    fn uniform(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn gauss(&mut self) -> f32 {
        ((-2.0 * self.uniform().ln()).sqrt() * (std::f64::consts::TAU * self.uniform()).cos()) as f32
    }
}

/// Sposta in frequenza l'audio di `shift(t)` Hz (t in secondi).
fn frequency_shift(input: &[f32], shift: impl Fn(f64) -> f64) -> Vec<f32> {
    const TAPS: usize = 127;
    let mid = (TAPS - 1) / 2;
    let h: Vec<f64> = (0..TAPS)
        .map(|i| {
            let n = i as i64 - mid as i64;
            let w = 0.54 - 0.46 * (std::f64::consts::TAU * i as f64 / (TAPS - 1) as f64).cos();
            if n % 2 == 0 { 0.0 } else { 2.0 / (std::f64::consts::PI * n as f64) * w }
        })
        .collect();
    let mut out = Vec::with_capacity(input.len());
    let mut phase = 0.0f64;
    for n in 0..input.len() {
        let re = if n >= mid { f64::from(input[n - mid]) } else { 0.0 };
        let mut im = 0.0;
        for (k, c) in h.iter().enumerate() {
            if *c != 0.0 && n >= k {
                im += c * f64::from(input[n - k]);
            }
        }
        let t = n as f64 / f64::from(RATE);
        phase = (phase + std::f64::consts::TAU * shift(t) / f64::from(RATE)) % std::f64::consts::TAU;
        out.push((re * phase.cos() - im * phase.sin()) as f32);
    }
    out
}

#[derive(Debug, Default)]
struct Outcome {
    rows: u32,
    lost: u32,
    images: u32,
    last_offset: Option<f32>,
}

fn decode(audio: &[f32]) -> Outcome {
    let mut dec = Decoder::new(DecoderConfig::default());
    let mut out = Outcome::default();
    for chunk in audio.chunks(1024) {
        for ev in dec.push(chunk) {
            match ev {
                DecoderEvent::Finished(done) => {
                    out.rows += done.image.height;
                    out.images += 1;
                    out.last_offset = Some(done.offset_hz);
                }
                DecoderEvent::Lost => out.lost += 1,
                _ => {}
            }
        }
        if let Some(o) = dec.offset_hz() {
            out.last_offset = Some(o);
        }
    }
    out
}

fn scenario(mode: SstvMode, skip_header: bool, shift: impl Fn(f64) -> f64) -> Outcome {
    let spec = mode.spec();
    let img = test_image(spec.width, spec.height);
    let mut audio = encode_shifted(mode, &img, RATE, 0.0).expect("codifica");
    if skip_header {
        // si entra a ricezione gia' iniziata: niente VIS, il modo si riconosce dalle righe
        let skip = ((HEADER_SECONDS + spec.group_seconds() * 10.5) * f64::from(RATE)) as usize;
        audio.drain(..skip);
    }
    let mut padded = vec![0.0f32; RATE as usize / 2];
    padded.extend(audio);
    padded.extend(vec![0.0f32; RATE as usize * 8]);
    let mut shifted = frequency_shift(&padded, shift);
    let mut noise = Noise(5);
    for s in &mut shifted {
        *s += 0.05 * noise.gauss();
    }
    decode(&shifted)
}

/// Annota un controllo fallito (si misurano tutti i casi prima di fermarsi).
fn check(bad: &mut Vec<String>, ok: bool, what: &str) {
    if !ok {
        bad.push(what.to_string());
    }
}

fn mid_image(mode: SstvMode) -> f64 {
    0.5 + HEADER_SECONDS + mode.spec().image_seconds() / 2.0
}

#[test]
fn vfo_step_in_the_middle_of_the_picture_keeps_the_picture() {
    let mut bad = Vec::new();
    for mode in [SstvMode::Martin1, SstvMode::Scottie1, SstvMode::Robot36] {
        let h = mode.spec().height;
        for step in [-150.0, 150.0, 250.0] {
            let at = mid_image(mode);
            let got = scenario(mode, false, |t| if t < at { 0.0 } else { step });
            let label = format!("{} {:?}", mode.spec().name, got);
            eprintln!("{} salto {step:+} Hz a meta': righe {} di {h}, scartate {}, immagini {}, scarto finale {:?}", mode.spec().name, got.rows, got.lost, got.images, got.last_offset);
            check(&mut bad, got.lost == 0, &format!("{} {}", label, "scartata"));
            check(&mut bad, got.images == 1, &format!("{} {}", label, "immagini"));
            check(&mut bad, got.rows >= h * 95 / 100, &format!("{} {}", label, "righe"));
            check(&mut bad, got.last_offset.is_some_and(|o| (o - step as f32).abs() < 25.0), &format!("{} {}", label, "scarto"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn slow_drift_is_followed() {
    let mut bad = Vec::new();
    for mode in [SstvMode::Martin1, SstvMode::Scottie1] {
        let spec = mode.spec();
        let start = 0.5 + HEADER_SECONDS;
        let len = spec.image_seconds();
        let got = scenario(mode, false, |t| 200.0 * ((t - start) / len).clamp(0.0, 1.0));
        let label = format!("{} {:?}", mode.spec().name, got);
        eprintln!("{} deriva 0 -> +200 Hz: righe {} di {}, scartate {}, scarto finale {:?}", spec.name, got.rows, spec.height, got.lost, got.last_offset);
        check(&mut bad, got.lost == 0, &format!("{} {}", label, "scartata"));
        check(&mut bad, got.rows >= spec.height * 95 / 100, &format!("{} {}", label, "righe"));
        check(&mut bad, got.last_offset.is_some_and(|o| (o - 200.0).abs() < 30.0), &format!("{} {}", label, "scarto"));
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn initial_offset_without_vis_is_found() {
    let mut bad = Vec::new();
    for mode in [SstvMode::Martin1, SstvMode::Scottie1] {
        let spec = mode.spec();
        for off in [-100.0, -50.0, 50.0, 100.0] {
            let got = scenario(mode, true, |_| off);
            let label = format!("{} {:?}", mode.spec().name, got);
            eprintln!("{} scarto iniziale {off:+} Hz senza VIS: righe {}, scartate {}, scarto stimato {:?}", spec.name, got.rows, got.lost, got.last_offset);
            check(&mut bad, got.lost == 0, &format!("{} {}", label, "scartata"));
            check(&mut bad, got.rows >= (spec.height - 12) * 9 / 10, &format!("{} {}", label, "righe"));
            check(&mut bad, got.last_offset.is_some_and(|o| (o - off as f32).abs() < 20.0), &format!("{} {}", label, "scarto"));
        }
    }
    assert!(bad.is_empty(), "{bad:?}");
}
