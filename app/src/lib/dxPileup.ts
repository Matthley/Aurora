import type { DigiSignal } from '../api/schemas';
import { getBandSignals, onBandSignals } from './digiBand';

/**
 * Modo DXpedition di Aurora BIGScan (CW e RTTY): in un pile-up in split si capisce dove ascolta la DX. Si segue il testo del canale della DX:
 * quando manda il rapporto o la conferma a un nominativo («K1XYZ 5NN», «K1XYZ 599», «K1XYZ TU»), quel nominativo si cerca fra i canali della
 * banda sentiti negli ultimi 60 s: la sua frequenza e' quella dove la DX ascoltava. Senza un chiamante trovato vale l'indicazione «UP n».
 */

/** Un chiamante collegato dalla DX e la frequenza (tono audio, Hz) dove lo si sentiva. */
export interface PileupWorked {
  call: string;
  hz: number;
  /** Quando la DX gli ha risposto (ms). */
  at: number;
}

export interface PileupState {
  /** Il nominativo della DX; null = modo spento. */
  dx: string | null;
  /** La frequenza della DX (Hz audio), se la si sente. */
  dxHz: number | null;
  /** Gli ultimi chiamanti collegati, dal piu' recente. */
  worked: PileupWorked[];
  /** Dove si stima che la DX ascolti adesso (Hz audio); null = non si sa ancora. */
  estimateHz: number | null;
  /** La finestra di ascolto: dal piu' basso al piu' alto degli ultimi chiamanti (Hz). */
  windowHz: [number, number] | null;
  /** Lo spostamento fra un chiamante e il successivo: sale, scende o fermo. */
  drift: 'up' | 'down' | 'none';
  /** L'indicazione letta nel testo della DX («UP 2» = +2000 Hz), quando non c'e' un chiamante trovato. */
  hintHz: number | null;
}

/** Quanti chiamanti collegati si tengono. */
export const PILEUP_KEEP = 8;
/** Per quanto un nominativo sentito su un canale resta buono come chiamante (ms). */
const HEARD_MS = 60_000;
/** Sotto questa distanza dalla DX il chiamante e' in simplex: non dice niente dello split (Hz). */
const SIMPLEX_HZ = 40;

const CALL_SHAPE = /^(?=.*\d)(?=.*[A-Z])[A-Z0-9]{1,3}\d[A-Z0-9]{0,4}[A-Z]$/;
/** Le parole con cui la DX risponde a un chiamante: rapporto o conferma. */
const REPLY = /^(5[1-9N][9N]|5[1-9N][9N]TU|TU|R|RR)$/;

/** Il nominativo di una parola (`EA8/K1XYZ/P` -> `K1XYZ`), o null. */
function callOf(word: string): string | null {
  const call = word.split('/').sort((a, b) => b.length - a.length)[0] ?? '';
  return CALL_SHAPE.test(call) ? call : null;
}

function words(text: string): string[] {
  return text.toUpperCase().split(/[^A-Z0-9/-]+/).filter(Boolean);
}

/** Il testo nuovo di un canale rispetto a quello visto prima (il core manda la coda del testo, che scorre). */
export function newText(before: string, after: string): string {
  if (before === '') return after;
  if (after.startsWith(before)) return after.slice(before.length);
  // la coda e' scorsa: il pezzo piu' lungo della fine del vecchio che apre il nuovo
  for (let k = Math.min(before.length, after.length); k >= 4; k--) {
    if (after.startsWith(before.slice(before.length - k))) return after.slice(k);
  }
  return after === before ? '' : after;
}

/** «UP», «UP 2», «UP2», «UP 2-5», «DOWN 1» nel testo della DX: lo scarto in Hz (1 kHz della radio = 1000 Hz di tono), o null. */
export function splitHint(text: string): number | null {
  const ws = words(text);
  let found: number | null = null;
  for (let i = 0; i < ws.length; i++) {
    const m = /^(UP|DOWN|DWN|DN)(\d+(?:\.\d+)?(?:-\d+(?:\.\d+)?)?)?$/.exec(ws[i] ?? '');
    if (!m) continue;
    const sign = m[1] === 'UP' ? 1 : -1;
    const arg = m[2] ?? (/^\d+(?:\.\d+)?(?:-\d+(?:\.\d+)?)?$/.test(ws[i + 1] ?? '') ? ws[i + 1] : undefined);
    let khz = 1;
    if (arg !== undefined) {
      const [a, b] = arg.split('-').map(Number);
      if (a === undefined || !Number.isFinite(a)) continue;
      khz = b !== undefined && Number.isFinite(b) ? (a + b) / 2 : a;
      // «UP 5» in kHz; un numero grande e' un'altra cosa (un rapporto, un numero di serie)
      if (khz <= 0 || khz > 20) continue;
    }
    found = sign * khz * 1000;
  }
  return found;
}

/** La stima dagli ultimi chiamanti (dal piu' recente): l'ultimo, piu' il passo medio se gli ultimi passi vanno tutti nello stesso verso. */
export function estimate(worked: PileupWorked[]): Pick<PileupState, 'estimateHz' | 'windowHz' | 'drift'> {
  if (worked.length === 0) return { estimateHz: null, windowHz: null, drift: 'none' };
  const hz = worked.map((w) => w.hz);
  const last = hz[0] ?? 0;
  const recent = hz.slice(0, 4).reverse();
  const steps = recent.slice(1).map((h, i) => h - (recent[i] ?? h));
  const up = steps.length >= 2 && steps.every((s) => s > 5);
  const down = steps.length >= 2 && steps.every((s) => s < -5);
  const mean = steps.length > 0 ? steps.reduce((a, b) => a + b, 0) / steps.length : 0;
  const shown = hz.slice(0, 5);
  return {
    estimateHz: Math.round(up || down ? last + mean : last),
    windowHz: [Math.min(...shown), Math.max(...shown)],
    drift: up ? 'up' : down ? 'down' : 'none',
  };
}

/** Il tracciatore: riceve gli elenchi dei segnali della banda e tiene chiamanti collegati e stima. Puro, per le prove. */
export class PileupTracker {
  dx: string | null = null;
  private texts = new Map<number, string>();
  private heard = new Map<string, { hz: number; at: number }>();
  private dxTail = '';
  private worked: PileupWorked[] = [];
  private hint: number | null = null;
  private dxHz: number | null = null;
  /** Il primo elenco dopo la scelta della DX: fa solo da punto di partenza (il testo vecchio non conta). */
  private baseline = true;

  setDx(call: string | null): void {
    const c = call ? call.trim().toUpperCase() : null;
    if (c === this.dx) return;
    this.dx = c || null;
    this.dxTail = '';
    this.worked = [];
    this.hint = null;
    this.dxHz = null;
    this.texts.clear();
    this.heard.clear();
    this.baseline = true;
  }

  /** Il canale della DX: quello che la nomina come chi trasmette (CQ o DE), poi uno qualsiasi che la nomina. */
  private dxSignal(list: DigiSignal[]): DigiSignal | null {
    const dx = this.dx;
    if (!dx) return null;
    const has = (s: DigiSignal, own: boolean) => s.calls.some((c) => c.call === dx && c.gone !== true && (!own || c.role !== 'other'));
    return list.find((s) => has(s, true)) ?? list.find((s) => has(s, false)) ?? null;
  }

  update(list: DigiSignal[], now: number): void {
    const dxSig = this.dxSignal(list);
    if (dxSig) this.dxHz = dxSig.freq_hz;
    const fresh = new Map<number, string>();
    const dxNew: string[] = [];
    for (const s of list) {
      const added = newText(this.texts.get(s.id) ?? '', s.text);
      fresh.set(s.id, s.text);
      if (s === dxSig) {
        if (added) dxNew.push(added);
        continue;
      }
      // i nominativi sentiti adesso su questo canale (nel testo nuovo, e quelli che il core ha capito chi trasmette)
      for (const w of words(added)) {
        const c = callOf(w);
        if (c && c !== this.dx) this.heard.set(c, { hz: s.freq_hz, at: now });
      }
      if (!this.texts.has(s.id)) {
        for (const c of s.calls) if (c.stale !== true && c.gone !== true && c.call !== this.dx) this.heard.set(c.call, { hz: s.freq_hz, at: now });
      }
    }
    this.texts = fresh;
    for (const [c, h] of this.heard) if (now - h.at > HEARD_MS) this.heard.delete(c);
    if (this.baseline) {
      this.baseline = false;
      return;
    }
    if (dxNew.length > 0) this.readDx(dxNew.join(''), now);
  }

  /** Il testo nuovo della DX: le parole complete (seguite da uno spazio) si leggono; l'ultima, a meta', aspetta il giro dopo. */
  private readDx(added: string, now: number): void {
    const text = (this.dxTail + added).toUpperCase();
    const cut = Math.max(text.lastIndexOf(' '), text.lastIndexOf('\n'));
    if (cut < 0) {
      this.dxTail = text.slice(-60);
      return;
    }
    const done = text.slice(0, cut);
    this.dxTail = text.slice(cut + 1).slice(-60);
    const ws = words(done);
    for (let i = 0; i < ws.length; i++) {
      const call = callOf(ws[i] ?? '');
      if (!call || call === this.dx) continue;
      const next = ws[i + 1] ?? '';
      if (!REPLY.test(next)) continue;
      const h = this.heard.get(call);
      if (!h || now - h.at > HEARD_MS) continue;
      if (this.dxHz !== null && Math.abs(h.hz - this.dxHz) < SIMPLEX_HZ) continue;
      // la stessa risposta ripetuta («K1XYZ 5NN K1XYZ 5NN»): un chiamante solo
      if (this.worked[0]?.call === call && now - (this.worked[0]?.at ?? 0) < 30_000) continue;
      this.worked = [{ call, hz: h.hz, at: now }, ...this.worked].slice(0, PILEUP_KEEP);
    }
    const hint = splitHint(done);
    if (hint !== null) this.hint = hint;
  }

  state(): PileupState {
    const est = estimate(this.worked);
    return {
      dx: this.dx,
      dxHz: this.dxHz,
      worked: this.worked,
      ...est,
      hintHz: this.hint !== null && this.dxHz !== null ? Math.round(this.dxHz + this.hint) : null,
    };
  }
}

// lo stato condiviso della pagina: la DX scelta resta anche lasciando la pagina, come i segnali della banda
const tracker = new PileupTracker();
let current: PileupState = tracker.state();
const listeners = new Set<() => void>();

function publish(): void {
  current = tracker.state();
  for (const l of listeners) l();
}

onBandSignals(() => {
  if (!tracker.dx) return;
  tracker.update(getBandSignals(), Date.now());
  publish();
});

/** Sceglie la DX del pile-up (null: modo spento). */
export function setPileupDx(call: string | null): void {
  tracker.setDx(call);
  if (tracker.dx) tracker.update(getBandSignals(), Date.now());
  publish();
}

export function getPileup(): PileupState {
  return current;
}

export function onPileup(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** La frequenza dove trasmettere: la stima dai chiamanti, se no l'indicazione «UP n». */
export function pileupTarget(p: PileupState): number | null {
  return p.estimateHz ?? p.hintHz;
}
