/**
 * Riconoscimento delle stazioni in modo Fox/Hound di WSJT-X, come `fox_sender` del motore: il Fox manda il messaggio DXpedition a due Hound
 * `K1ABC RR73; W9XYZ <KH1/KH7Z> -08` (`i3=0 n3=1`). Le risposte multiple normali (MSHV) sono messaggi standard separati e non contano.
 */
export function foxSender(text: string): string | null {
  const [first, after] = text.trim().toUpperCase().split('; ');
  if (first === undefined || after === undefined) return null;
  const a = first.split(/\s+/);
  const b = after.split(/\s+/);
  if (a.length !== 2 || a[1] !== 'RR73' || b.length !== 3) return null;
  const fox = b[1]!.replace(/^</, '').replace(/>$/, '');
  return fox === '' || fox === '...' ? null : fox;
}

/** Una Fox vera trasmette sotto i 1000 Hz (gli Hound sopra): un messaggio doppio piu' in alto e' una risposta multipla di una stazione normale. */
export const FOX_MAX_HZ = 1000;

/**
 * I nominativi che nei messaggi ricevuti trasmettono come Fox (messaggio DXpedition sotto i 1000 Hz, se la frequenza e' nota).
 * Il messaggio DXpedition da solo non basta: MSHV in modo normale con piu' flussi (multi-stream) manda anche lui righe `A RR73; B <DX> -nn`
 * (issue #68, 3X4U). Una Fox di WSJT-X trasmette solo sotto i 1000 Hz e i suoi Hound chiamano solo fra 1000 e 3000 Hz
 * (`QRandomGenerator::bounded (1000, 2999)` di WSJT-X): chi trasmette sopra i 1000 Hz, o e' chiamato da una stazione sotto i 1000 Hz, non e' una Fox.
 */
export function foxCalls(items: Array<string | { text: string; freq_hz?: number }>): Set<string> {
  const out = new Set<string>();
  // chi manda R+rapporto o RRR fa la parte di chi chiama (Hound o stazione normale): una Fox non li manda mai
  const notFox = new Set<string>();
  for (const item of items) {
    const text = typeof item === 'string' ? item : item.text;
    const w = text.trim().toUpperCase().split(/\s+/);
    if (w.length === 3 && w[0] !== 'CQ' && /^(R[+-]\d{2}|RRR)$/.test(w[2]!)) notFox.add(w[1]!.replace(/[<>]/g, ''));
    const hz = typeof item === 'string' ? undefined : item.freq_hz;
    if (hz !== undefined && hz >= FOX_MAX_HZ) {
      // una Fox non trasmette mai qui: chi manda una riga sopra i 1000 Hz e' una stazione normale (o un Hound)
      const s = lineSender(text);
      if (s) notFox.add(s.toUpperCase());
      continue;
    }
    if (hz !== undefined && w.length >= 2 && w.length <= 3 && w[0] !== 'CQ' && !text.includes(';')) {
      // una stazione la chiama o le risponde da sotto i 1000 Hz: in Fox/Hound i Hound non trasmettono li'
      notFox.add(w[0]!.replace(/[<>]/g, ''));
    }
    const f = foxSender(text);
    if (f) out.add(f);
  }
  for (const c of notFox) out.delete(c);
  return out;
}

/** Chi trasmette una riga: il Fox di una riga DXpedition, chi chiama CQ, il secondo nominativo di un messaggio diretto. */
export function lineSender(text: string): string | null {
  const fox = foxSender(text);
  if (fox) return fox;
  const w = text.trim().toUpperCase().split(/\s+/);
  if (w[0] === 'CQ') return (w.length > 2 && w[1]!.length <= 4 && !/\d/.test(w[1]!) ? w[2] : w[1])?.replace(/[<>]/g, '') ?? null;
  return w.length >= 2 ? w[1]!.replace(/[<>]/g, '') : null;
}
