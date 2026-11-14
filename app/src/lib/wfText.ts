/** Le ultime parole di ogni canale scritte sulla cascata sotto il nominativo, e lo storico breve per il suggerimento. */

export type WordKind = 'call' | 'key' | 'plain';

/** Un pezzo della riga: testo, tipo (nominativo, parola chiave, normale) e incerto. */
export interface WordChunk {
  text: string;
  kind: WordKind;
  unsure: boolean;
}

/** Le parole che si evidenziano come scambio di contest o di QSO. */
const KEYS = new Set(['CQ', 'TEST', 'DE', 'TU', '5NN', '599', 'K', 'KN', 'BK', 'R', 'RR', 'TNX', '73', 'QRZ', 'AGN', 'SK', 'EE']);
const CALL_RE = /^(?:[A-Z0-9]{1,3}\/)?(?:[A-Z]{1,2}|\d[A-Z]{1,2}|[A-Z]\d)\d[A-Z]{1,4}(?:\/[A-Z0-9]{1,4})?$/;

export function wordKind(word: string, calls: string[] = []): WordKind {
  const w = word.toUpperCase();
  if (KEYS.has(w)) return 'key';
  if (calls.includes(w) || CALL_RE.test(w)) return 'call';
  return 'plain';
}

/**
 * La coda del testo di un canale (al massimo `max` caratteri, tagliata a inizio parola con «…»), divisa in pezzi.
 * Restituisce `null` se non c'e' niente da mostrare o se i caratteri sicuri sono troppo pochi (testo solo incerto o rumore).
 */
export function tailChunks(text: string, unsure: number[], calls: string[] = [], max = 26): WordChunk[] | null {
  const chars = Array.from(text);
  const set = new Set(unsure);
  // si tolgono gli spazi in fondo e si ricorda la posizione vera di ogni carattere
  let end = chars.length;
  while (end > 0 && (chars[end - 1] ?? '').trim() === '') end--;
  if (end === 0) return null;
  let start = Math.max(0, end - max);
  let cut = start > 0;
  if (cut) {
    // si comincia da una parola intera se ce n'e' una vicina
    const sp = chars.slice(start, end).findIndex((c) => c.trim() === '');
    if (sp >= 0 && sp < 10) start += sp + 1;
  }
  while (start < end && (chars[start] ?? '').trim() === '') start++;
  if (start >= end) return null;
  const visible = chars.slice(start, end);
  const solid = visible.filter((c, i) => c.trim() !== '' && !set.has(start + i)).length;
  const letters = visible.filter((c) => c.trim() !== '').length;
  if (solid < 3 || solid < letters * 0.4) return null;
  cut = start > 0;

  const out: WordChunk[] = [];
  if (cut) out.push({ text: '…', kind: 'plain', unsure: false });
  const push = (s: string, kind: WordKind, u: boolean) => {
    const last = out[out.length - 1];
    if (last && last.kind === kind && last.unsure === u && last.text !== '…') last.text += s;
    else out.push({ text: s, kind, unsure: u });
  };
  // parola per parola: il tipo vale per tutta la parola, l'incertezza carattere per carattere
  let i = 0;
  while (i < visible.length) {
    const c = visible[i] ?? '';
    if (c.trim() === '') {
      push(' ', 'plain', false);
      while (i < visible.length && (visible[i] ?? '').trim() === '') i++;
      continue;
    }
    let j = i;
    while (j < visible.length && (visible[j] ?? '').trim() !== '') j++;
    const kind = wordKind(visible.slice(i, j).join(''), calls);
    for (let k = i; k < j; k++) push(visible[k] ?? '', kind, set.has(start + k));
    i = j;
  }
  return out;
}

/** Quanto resta lo storico per il suggerimento (ms). */
export const HISTORY_MS = 180_000;
/** Dopo quanto senza testo nuovo la riga sbiadisce, e dopo quanto sparisce (ms). */
export const QUIET_MS = 20_000;
export const GONE_MS = 60_000;

interface Track {
  text: string;
  changed: number;
  parts: { at: number; text: string }[];
}

/** La parte nuova di `next` rispetto a `prev` (il testo del canale scorre: si cerca la sovrapposizione piu' lunga). */
export function newPart(prev: string, next: string): string {
  if (prev === '') return next;
  if (next.startsWith(prev)) return next.slice(prev.length);
  for (let n = Math.min(prev.length, next.length); n >= 4; n--) {
    if (next.startsWith(prev.slice(prev.length - n))) return next.slice(n);
  }
  const at = next.indexOf(prev.slice(-12));
  if (prev.length >= 12 && at >= 0) return next.slice(at + 12);
  return next;
}

/** Lo storico del testo di ogni canale (per id del segnale): quando e' cambiato l'ultima volta e cosa si e' letto negli ultimi minuti. */
export class TextHistory {
  private tracks = new Map<number, Track>();

  update(id: number, text: string, now = Date.now()): void {
    const t = this.tracks.get(id);
    if (!t) {
      this.tracks.set(id, { text, changed: now, parts: text.trim() ? [{ at: now, text }] : [] });
      return;
    }
    if (t.text === text) return;
    const add = newPart(t.text, text);
    t.text = text;
    t.changed = now;
    if (add) t.parts.push({ at: now, text: add });
    t.parts = t.parts.filter((p) => now - p.at <= HISTORY_MS);
  }

  /** Toglie i canali spariti dall'elenco. */
  keep(ids: Set<number>): void {
    for (const id of this.tracks.keys()) if (!ids.has(id)) this.tracks.delete(id);
  }

  /** Da quanti ms il canale non scrive niente di nuovo. */
  idle(id: number, now = Date.now()): number {
    const t = this.tracks.get(id);
    return t ? now - t.changed : Infinity;
  }

  /** Il testo degli ultimi minuti, spazi ripetuti ridotti. */
  history(id: number, now = Date.now()): string {
    const t = this.tracks.get(id);
    if (!t) return '';
    return t.parts
      .filter((p) => now - p.at <= HISTORY_MS)
      .map((p) => p.text)
      .join('')
      .replace(/\s+/g, ' ')
      .trim();
  }
}
