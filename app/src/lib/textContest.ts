/**
 * Il contest dei modi testuali scelto dal menu Contest: finestra di date dei doppi, categorie valide, doppi senza banda o modo, macro
 * del contest unite a quelle del modo, numero da mandare e cattura dello scambio ricevuto (numero o dominio) dal testo.
 */
import type { Domain, TextContest } from '../api/contestTesto';
import { buildLogIndex, contestStatus, type LogStatus } from './logStatus';
import { qsoTimeMs } from './logWindow';
import type { Macro } from './macroLang';

/** Una data `aaaammgghhmmss` (UTC) in ms, o `null`. */
export function contestDateMs(d: string): number | null {
  const m = /^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})$/.exec(d.trim());
  if (!m) return null;
  const ms = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]), Number(m[4]), Number(m[5]), Number(m[6]));
  return Number.isFinite(ms) ? ms : null;
}

const withYear = (ms: number, years: number) => {
  const d = new Date(ms);
  return Date.UTC(d.getUTCFullYear() + years, d.getUTCMonth(), d.getUTCDate(), d.getUTCHours(), d.getUTCMinutes(), d.getUTCSeconds());
};

/**
 * La finestra del contest per l'anno di adesso: le date del file valgono ogni anno (conta il giorno e l'ora), quindi si portano
 * nell'anno corrente mantenendo la durata. Senza date: `null` (contano tutti i QSO).
 */
export function contestWindow(c: Pick<TextContest, 'start' | 'end'>, nowMs: number): { from: number | null; to: number | null } | null {
  const a = contestDateMs(c.start);
  const b = contestDateMs(c.end);
  if (a == null && b == null) return null;
  const ref = a ?? b ?? nowMs;
  const shift = new Date(nowMs).getUTCFullYear() - new Date(ref).getUTCFullYear();
  return { from: a == null ? null : withYear(a, shift), to: b == null ? null : withYear(b, shift) };
}

export type ModeCategory = 'cw' | 'phone' | 'digital';

/** La categoria di un modo: CW, fonia (SSB, AM, FM) o digitale (tutto il resto). */
export function modeCategory(mode: string): ModeCategory {
  const m = mode.trim().toUpperCase();
  if (m === 'CW') return 'cw';
  if (['SSB', 'USB', 'LSB', 'AM', 'FM', 'PHONE'].includes(m)) return 'phone';
  return 'digital';
}

/** Il modo conta per il contest? Nessuna categoria scelta = tutte. */
export function categoryValid(c: Pick<TextContest, 'valid_cw' | 'valid_phone' | 'valid_digital'>, mode: string): boolean {
  if (!c.valid_cw && !c.valid_phone && !c.valid_digital) return true;
  const k = modeCategory(mode);
  return k === 'cw' ? c.valid_cw : k === 'phone' ? c.valid_phone : c.valid_digital;
}

type Q = { call: string; band: string; mode: string; date: string; time: string };

/** I QSO del log che contano per il contest: dentro la finestra e di una categoria valida. */
export function contestQsos<T extends Q>(qsos: readonly T[], c: TextContest, nowMs: number): T[] {
  const w = contestWindow(c, nowMs);
  return qsos.filter((q) => {
    if (!categoryValid(c, q.mode)) return false;
    if (!w) return true;
    const t = qsoTimeMs(q);
    if (!Number.isFinite(t)) return false;
    return !(w.from != null && t < w.from) && !(w.to != null && t > w.to);
  });
}

/**
 * Il segno di BIGScan con un contest dei modi testuali: doppio se lo stesso nominativo e' gia' nei QSO del contest con la stessa banda
 * (salvo «doppio su ogni banda») e lo stesso modo (salvo «doppio in ogni modo»); altrimenti le novita' rispetto ai QSO del contest.
 */
export function textContestStatus(qsos: readonly Q[], c: TextContest, call: string, band: string | null, mode: string): LogStatus {
  const key = call.trim().toUpperCase();
  if (!categoryValid(c, mode)) return null;
  const dupe = qsos.some((q) => q.call.trim().toUpperCase() === key && (c.dupe_no_band || !band || q.band.toLowerCase() === band.toLowerCase()) && (c.dupe_no_mode || q.mode.toUpperCase() === mode.toUpperCase()));
  if (dupe) return 'dupe';
  return contestStatus(null, buildLogIndex(qsos.map((q) => ({ ...q, country: '' }))), call, band, mode);
}

/** Le macro in uso con il contest: quelle del contest sostituiscono le altre sugli stessi tasti; senza «Unisci» restano solo loro. */
export function withContestMacros(effective: ReadonlyMap<string, Macro>, contest: readonly Macro[] | undefined, merge: boolean): Map<string, Macro> {
  if (!contest || contest.length === 0) return new Map(effective);
  const out = merge ? new Map(effective) : new Map<string, Macro>();
  for (const m of contest) out.set(m.name, m);
  return out;
}

/** Il primo numero da mandare: le cifre di «Numero da mandare» (almeno 1). */
export function startSerial(nrs: string): number {
  const n = Number((nrs.match(/\d+/) ?? [''])[0]);
  return Number.isInteger(n) && n >= 1 ? Math.min(n, 99999) : 1;
}

/** Il dominio di una parola: prima nelle sezioni il cui prefisso inizia il nominativo, poi in tutto l'elenco. */
export function domainLookup(d: Domain | null | undefined, word: string, call: string): string | null {
  if (!d) return null;
  const w = word.trim().toUpperCase();
  const c = call.trim().toUpperCase();
  if (!w) return null;
  const inSection = d.entries.find((e) => e.key === w && e.section !== '' && c.startsWith(e.section));
  return (inSection ?? d.entries.find((e) => e.key === w))?.value ?? null;
}

const RST = /^(5[1-9][1-9]|5[1-9]|5NN)$/i;

/** Il testo senza le righe di quello che abbiamo trasmesso noi (l'eco «» …» della trasmissione e il testo mandato). */
export function withoutOwnEcho(text: string): string {
  return text
    .split('\n')
    .filter((l) => !l.startsWith('»'))
    .join('\n');
}

/**
 * Lo scambio ricevuto preso dal testo: le parole dopo l'ultima volta che compare il nominativo del QSO. Prima il dominio (se l'elenco
 * c'e'), poi il numero (il rapporto 599/59 davanti si salta). `null` se non c'e' niente da prendere. Il nostro testo trasmesso (righe
 * «» …»: il testo mandato e l'eco della trasmissione) non conta: con l'eco accesa il nominativo del corrispondente seguito dal nostro
 * numero e' proprio quello che trasmettiamo. Il numero uguale al nostro non si scarta (all'inizio tutti mandano 001).
 */
export function grabExchange(text: string, call: string, opts: { number: boolean; domain: Domain | null | undefined }): string | null {
  const c = call.trim().toUpperCase();
  if (c.length < 3) return null;
  const words = withoutOwnEcho(text).toUpperCase().split(/[^A-Z0-9/]+/).filter(Boolean);
  let at = -1;
  for (let i = words.length - 1; i >= 0; i--) if (words[i] === c) { at = i; break; }
  if (at < 0) return null;
  const after = words.slice(at + 1, at + 9).filter((w) => w !== 'DE' && w !== 'TU' && w !== 'UR' && w !== 'NR');
  if (opts.domain) for (const w of after) {
    if (RST.test(w)) continue;
    const v = domainLookup(opts.domain, w, c);
    if (v) return v;
  }
  if (opts.number) {
    const nums = after.filter((w) => /^\d{1,5}$/.test(w));
    const n = nums.find((w) => !RST.test(w)) ?? null;
    return n;
  }
  return null;
}
