//! Correzione di sintonia a mano: ridisegna l'immagine dal segnale conservato, anche dopo la fine, e sostituisce l'AFC nella lettura.

use mode_sstv::{Decoder, DecoderConfig, DecoderEvent, RATE, RgbImage, SstvMode, decoder::TUNE_STEP_HZ, encoder::encode_shifted};

fn test_image(w: u32, h: u32) -> RgbImage {
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (f64::from(x) / f64::from(w), f64::from(y) / f64::from(h));
            let r = 40.0 + 180.0 * fx;
            let g = 60.0 + 150.0 * fy;
            let b = 200.0 - 140.0 * fx * fy;
            data.extend([r, g, b].map(|v: f64| v.clamp(0.0, 255.0) as u8));
        }
    }
    RgbImage { width: w, height: h, data }
}

fn psnr(a: &RgbImage, b: &RgbImage) -> f64 {
    let rows = a.height.min(b.height) as usize;
    let n = rows * a.width as usize * 3;
    let mse: f64 = a.data[..n].iter().zip(&b.data[..n]).map(|(&x, &y)| (f64::from(x) - f64::from(y)).powi(2)).sum::<f64>() / n as f64;
    10.0 * (255.0f64.powi(2) / mse.max(1e-9)).log10()
}

/// Un segnale spostato di +100 Hz: con la correzione a +100 Hz (16 passi) l'immagine ridisegnata e' quella giusta, con la correzione
/// sbagliata no, e tornando a 0 si rilegge con l'AFC.
#[test]
fn la_correzione_a_mano_ridisegna_l_immagine_gia_ricevuta() {
    let mode = SstvMode::Robot36;
    let spec = mode.spec();
    let img = test_image(spec.width, spec.height);
    let mut audio = vec![0.0f32; RATE as usize / 2];
    audio.extend(encode_shifted(mode, &img, RATE, 100.0).expect("codifica"));
    audio.extend(vec![0.0f32; RATE as usize]);
    let mut dec = Decoder::new(DecoderConfig::default());
    let mut finished = None;
    for chunk in audio.chunks(4096) {
        for ev in dec.push(chunk) {
            if let DecoderEvent::Finished(done) = ev {
                finished = Some(done);
            }
        }
    }
    let done = finished.expect("immagine finita");
    let auto = psnr(&img, &done.image);
    assert!(auto > 25.0, "con l'AFC: {auto:.1} dB");

    let steps = (100.0 / TUNE_STEP_HZ).round() as i32;
    let (m, wrong, live) = dec.set_tune(-steps).expect("ridisegno con la correzione sbagliata");
    assert_eq!(m, mode);
    assert!(!live, "l'immagine e' gia' finita");
    let bad = psnr(&img, &wrong);
    assert!(bad < auto - 10.0, "correzione sbagliata: {bad:.1} dB contro {auto:.1}");

    let (_, right, _) = dec.set_tune(steps).expect("ridisegno con la correzione giusta");
    assert_eq!((right.width, right.height), (done.image.width, done.image.height));
    let good = psnr(&img, &right);
    assert!(good > 25.0, "correzione a +100 Hz: {good:.1} dB");
    assert!((dec.tune_hz() - 100.0).abs() < 0.01);

    let (_, back, _) = dec.set_tune(0).expect("ritorno all'AFC");
    assert!((psnr(&img, &back) - auto).abs() < 0.5);
}

/// La correzione si ferma a ±32 passi (±200 Hz) e senza immagine non ridisegna nulla.
#[test]
fn la_correzione_ha_un_limite_e_senza_immagine_non_ridisegna() {
    let mut dec = Decoder::new(DecoderConfig::default());
    assert!(dec.set_tune(100).is_none());
    assert!((dec.tune_hz() - 200.0).abs() < 0.01);
    assert!(dec.set_tune(-100).is_none());
    assert!((dec.tune_hz() + 200.0).abs() < 0.01);
}

/// L'istogramma mette un tono al suo livello (1900 Hz = 128, 6,25 Hz per livello) e si svuota quando il tono sparisce.
#[test]
fn l_istogramma_mette_il_tono_al_suo_livello_e_poi_si_svuota() {
    let mut dec = Decoder::new(DecoderConfig::default());
    let tone: Vec<f32> = (0..RATE as usize / 2).map(|n| (std::f32::consts::TAU * 1500.0 * n as f32 / RATE as f32).sin()).collect();
    dec.push(&tone);
    let h = dec.histogram();
    assert_eq!(h.len(), 256);
    // i cassetti pieni arrivano al tetto: il centro e' la media pesata
    let total: f64 = h.iter().map(|&v| f64::from(v)).sum();
    let peak = h.iter().enumerate().map(|(i, &v)| i as f64 * f64::from(v)).sum::<f64>() / total;
    assert!((peak - 64.0).abs() < 1.0, "1500 Hz al livello {peak:.1}");
    assert!(h[32] == 0 && h[128] == 0, "solo intorno al tono");
    dec.push(&vec![0.0f32; RATE as usize * 3]);
    // il silenzio da' valori a caso: i cassetti del tono devono essere scesi
    assert!(dec.histogram()[64] < h[64]);
}
