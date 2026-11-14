/** Un tratto di una riga della finestra di ricezione: carattere incerto (`weak`) e/o dentro una parola cercata (`hit`). */
export interface RxPiece {
  start: number;
  text: string;
  weak: boolean;
  hit: boolean;
  /** Il tratto apre una parola trovata (per saltare da un risultato all'altro). */
  first: boolean;
}

/** Le posizioni [inizio, fine) di `query` in `line`, senza badare a maiuscole e minuscole, senza sovrapposizioni. */
export function findMatches(line: string, query: string): Array<[number, number]> {
  const q = query.trim().toUpperCase();
  if (q === '') return [];
  const up = line.toUpperCase();
  const out: Array<[number, number]> = [];
  for (let at = up.indexOf(q); at >= 0; at = up.indexOf(q, at + q.length)) out.push([at, at + q.length]);
  return out;
}

/** La riga divisa in tratti con lo stesso stile: `mask` ha '1' sui caratteri incerti, le parole trovate di `query` vanno evidenziate. */
export function rxPieces(line: string, mask: string, query: string): RxPiece[] {
  const hit = new Uint8Array(line.length);
  const starts = new Set<number>();
  for (const [a, b] of findMatches(line, query)) {
    hit.fill(1, a, b);
    starts.add(a);
  }
  const pieces: RxPiece[] = [];
  let start = 0;
  for (let k = 1; k <= line.length; k++) {
    if (k === line.length || mask[k] !== mask[start] || hit[k] !== hit[start] || starts.has(k)) {
      pieces.push({ start, text: line.slice(start, k), weak: mask[start] === '1', hit: hit[start] === 1, first: starts.has(start) });
      start = k;
    }
  }
  return pieces;
}

/** Il prossimo (o precedente, `back`) indice fra `count` risultati partendo da `current` (-1 = nessuno), con il giro. */
export function nextHit(current: number, count: number, back: boolean): number {
  if (count <= 0) return -1;
  if (current < 0) return back ? count - 1 : 0;
  return (current + (back ? count - 1 : 1)) % count;
}
