/**
 * I QTC del WAE DX Contest (`docs/contest-qtc.md`): le righe da mandare (intestazione `QTC n/m`, righe `ora-nominativo-numero`, richieste
 * di ripetizione) come testo delle macro, e la lettura della serie ricevuta dalla finestra di ricezione.
 */

/** Una riga di QTC: ora `HHMM`, nominativo e numero del QSO riportato. */
export interface QtcLine {
  ora: string;
  call: string;
  nr: string;
}

/** La riga come si trasmette: `1332-K5XYZ-112`. */
export function formatQtcLine(r: QtcLine): string {
  return `${r.ora.trim()}-${r.call.trim().toUpperCase()}-${r.nr.trim()}`;
}

/** L'intestazione della serie, due volte: `QTC 3/7 QTC 3/7`. */
export function qtcHeader(serie: number, n: number): string {
  return `QTC ${serie}/${n} QTC ${serie}/${n}`;
}

/** Gli script delle macro che la finestra QTC manda (`<TX>` ... `<RX>`). `call` e' la stazione, `my` il nostro nominativo. */
export const qtcScripts = {
  /** Chi manda: l'intestazione della serie. */
  header: (serie: number, n: number) => `<TX> ${qtcHeader(serie, n)} <RX>`,
  /** Una riga sola (anche per ripeterla). */
  line: (r: QtcLine) => `<TX> ${formatQtcLine(r)} ${formatQtcLine(r)} <RX>`,
  /** Tutte le righe, una per riga, e la richiesta di conferma. */
  all: (serie: number, rows: readonly QtcLine[]) => `<TX> ${qtcHeader(serie, rows.length)}<CRLF>${rows.map(formatQtcLine).join('<CRLF>')}<CRLF>QSL? <RX>`,
  qsl: () => '<TX> QSL? <RX>',
  /** Chi riceve: pronto a ricevere, ripetizioni e conferma. */
  qrv: (my: string) => `<TX> QRV QRV DE ${my.toUpperCase()} <RX>`,
  agn: (n: number) => `<TX> AGN ${n} AGN ${n} <RX>`,
  agnAll: () => '<TX> AGN ALL QTC AGN ALL QTC <RX>',
  agnHeader: () => '<TX> AGN QTC NR AGN QTC NR <RX>',
  confirm: (call: string, my: string) => `<TX> ${call.toUpperCase()} QSL ALL QTC TU DE ${my.toUpperCase()} <RX>`,
};

/** L'intestazione `QTC n/m` letta dal testo (l'ultima), o `null`. */
export function parseQtcHeader(text: string): { serie: number; n: number } | null {
  const all = [...text.toUpperCase().matchAll(/\bQTC\s*:?\s*(\d{1,3})\s*\/\s*(\d{1,2})\b/g)];
  const m = all[all.length - 1];
  if (!m) return null;
  const serie = Number(m[1]);
  const n = Number(m[2]);
  return serie >= 1 && n >= 1 && n <= 10 ? { serie, n } : null;
}

const LINE = /\b([0-2]\d[0-5]\d)\s*[- ]\s*([A-Z0-9/]{3,})\s*[- ]\s*(\d{1,4})\b/g;
const isCall = (c: string) => /\d/.test(c) && /[A-Z]/.test(c);

/**
 * La serie ricevuta presa dal testo della ricezione: l'ultima intestazione `QTC n/m` e le righe `ora nominativo numero` che la seguono
 * (separate da spazi, trattini o barre; le righe ripetute si contano una volta). Senza intestazione si prendono le righe di tutto il testo.
 * Le righe del nostro testo trasmesso (`»`) non contano.
 */
export function parseQtcSeries(text: string): { serie: number | null; n: number | null; rows: QtcLine[] } {
  const own = text
    .split('\n')
    .filter((l) => !l.startsWith('»'))
    .join('\n')
    .toUpperCase();
  const heads = [...own.matchAll(/\bQTC\s*:?\s*(\d{1,3})\s*\/\s*(\d{1,2})\b/g)];
  const last = heads[heads.length - 1];
  const from = last ? (last.index ?? 0) + last[0].length : 0;
  const head = parseQtcHeader(own);
  const rows: QtcLine[] = [];
  const seen = new Set<string>();
  for (const m of own.slice(from).matchAll(LINE)) {
    const r = { ora: m[1] ?? '', call: m[2] ?? '', nr: (m[3] ?? '').padStart(3, '0') };
    const k = formatQtcLine(r);
    if (!isCall(r.call) || seen.has(k)) continue;
    seen.add(k);
    rows.push(r);
    if (head && rows.length >= head.n) break;
  }
  return { serie: head?.serie ?? null, n: head?.n ?? null, rows };
}

/** La riga e' completa? Ora HHMM valida, nominativo con una cifra, numero di cifre. */
export function qtcLineValid(r: QtcLine): boolean {
  const ora = /^([01]\d|2[0-3])[0-5]\d$/.test(r.ora.trim());
  return ora && /^[A-Z0-9/]*\d[A-Z0-9/]*$/i.test(r.call.trim()) && r.call.trim().length >= 3 && /^\d{1,4}$/.test(r.nr.trim());
}
