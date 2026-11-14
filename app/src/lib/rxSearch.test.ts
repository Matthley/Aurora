import { describe, expect, it } from 'vitest';
import { findMatches, nextHit, rxPieces } from './rxSearch';

describe('ricerca nella finestra di ricezione', () => {
  it('trova tutte le occorrenze senza badare alle maiuscole', () => {
    expect(findMatches('cq iu3abc IU3ABC de Iu3Abc', 'iu3abc')).toEqual([[3, 9], [10, 16], [20, 26]]);
    expect(findMatches('qualcosa', '  ')).toEqual([]);
    expect(findMatches('aaaa', 'aa')).toEqual([[0, 2], [2, 4]]);
  });

  it('divide la riga fra caratteri incerti e parole trovate', () => {
    const p = rxPieces('DE IU3ABC K', '00000110000', 'iu3abc');
    expect(p.map((x) => [x.text, x.weak, x.hit, x.first])).toEqual([
      ['DE ', false, false, false],
      ['IU', false, true, true],
      ['3A', true, true, false],
      ['BC', false, true, false],
      [' K', false, false, false],
    ]);
  });

  it('due parole trovate attaccate restano separate', () => {
    expect(rxPieces('abab', '', 'ab').map((x) => [x.text, x.first])).toEqual([['ab', true], ['ab', true]]);
  });

  it('salta avanti e indietro con il giro', () => {
    expect(nextHit(-1, 3, false)).toBe(0);
    expect(nextHit(-1, 3, true)).toBe(2);
    expect(nextHit(2, 3, false)).toBe(0);
    expect(nextHit(0, 3, true)).toBe(2);
    expect(nextHit(0, 0, false)).toBe(-1);
  });
});
