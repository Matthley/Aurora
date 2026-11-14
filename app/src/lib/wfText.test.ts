import { describe, expect, it } from 'vitest';
import { TextHistory, newPart, tailChunks, wordKind } from './wfText';

const join = (c: ReturnType<typeof tailChunks>) => (c ?? []).map((x) => x.text).join('');

describe('wordKind', () => {
  it('riconosce nominativi e parole dello scambio', () => {
    expect(wordKind('EC7YY')).toBe('call');
    expect(wordKind('IU3ABC/P')).toBe('call');
    expect(wordKind('5NN')).toBe('key');
    expect(wordKind('599')).toBe('key');
    expect(wordKind('tu')).toBe('key');
    expect(wordKind('123')).toBe('plain');
    expect(wordKind('HELLO')).toBe('plain');
  });
});

describe('tailChunks', () => {
  it('evidenzia nominativi e parole chiave', () => {
    const c = tailChunks('CQ TEST EC7YY', []);
    expect(c).toEqual([
      { text: 'CQ', kind: 'key', unsure: false },
      { text: ' ', kind: 'plain', unsure: false },
      { text: 'TEST', kind: 'key', unsure: false },
      { text: ' ', kind: 'plain', unsure: false },
      { text: 'EC7YY', kind: 'call', unsure: false },
    ]);
  });

  it('tiene solo la coda, da inizio parola, con i puntini', () => {
    const c = tailChunks('QRL? QRL? CQ TEST EC7YY EC7YY TEST 5NN 123 TU', [], [], 20);
    const s = join(c);
    expect(s.startsWith('…')).toBe(true);
    expect(s.endsWith('5NN 123 TU')).toBe(true);
    expect(s.length).toBeLessThanOrEqual(21);
    expect(s.charAt(1)).not.toBe(' ');
  });

  it('segna i caratteri incerti e riduce gli spazi', () => {
    const c = tailChunks('5NN   12E', [8]);
    expect(join(c)).toBe('5NN 12E');
    expect(c?.at(-1)).toEqual({ text: 'E', kind: 'plain', unsure: true });
  });

  it('niente riga se il testo e\' vuoto o quasi tutto incerto', () => {
    expect(tailChunks('   ', [])).toBeNull();
    expect(tailChunks('EEE TTT', [0, 1, 2, 4, 5, 6])).toBeNull();
    expect(tailChunks('E', [])).toBeNull();
  });
});

describe('storico del testo', () => {
  it('trova la parte nuova del testo che scorre', () => {
    expect(newPart('', 'CQ')).toBe('CQ');
    expect(newPart('CQ TE', 'CQ TEST')).toBe('ST');
    expect(newPart('ABCD CQ TEST', 'CQ TEST EC7YY')).toBe(' EC7YY');
  });

  it('ricorda gli ultimi tre minuti e il tempo di silenzio', () => {
    const h = new TextHistory();
    h.update(1, 'CQ TEST', 0);
    h.update(1, 'CQ TEST EC7YY', 1_000);
    expect(h.history(1, 2_000)).toBe('CQ TEST EC7YY');
    expect(h.idle(1, 21_000)).toBe(20_000);
    h.update(1, 'CQ TEST EC7YY 5NN', 200_000);
    expect(h.history(1, 200_000)).toBe('5NN');
    h.keep(new Set());
    expect(h.idle(1)).toBe(Infinity);
  });
});
