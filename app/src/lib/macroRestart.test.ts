import { describe, expect, it } from 'vitest';
import { createLatestSequencer, restartBox } from './macroRestart';

/** Un motore finto: la coda non ancora modulata si sostituisce a ogni ripartenza, `taken` caratteri sono gia' partiti. */
function fakeEngine() {
  let out = '';
  let pending = '';
  return {
    restart(text: string) {
      const kept = out.length;
      pending = text;
      return Promise.resolve(kept);
    },
    modulate(n: number) {
      out += pending.slice(0, n);
      pending = pending.slice(n);
    },
    finish() {
      out += pending;
      pending = '';
      return out;
    },
  };
}

describe('macro ripremuta', () => {
  const F2 = 'CQ TEST IU3ABC IU3ABC';

  it('F2 tre volte: parte solo l\'ultima intera, dall\'inizio', async () => {
    const eng = fakeEngine();
    const seq = createLatestSequencer();
    let box = '';
    const press = (text: string) => {
      const r = seq.run(() => eng.restart(text));
      return r.done.then((kept) => {
        box = restartBox(box, kept, text);
        return r.gen;
      });
    };
    const g1 = await press(F2);
    eng.modulate(5);
    const g2 = await press(F2);
    eng.modulate(3);
    const g3 = await press(F2);
    expect(seq.isLatest(g1)).toBe(false);
    expect(seq.isLatest(g2)).toBe(false);
    expect(seq.isLatest(g3)).toBe(true);
    expect(eng.finish()).toBe('CQ TECQ ' + F2);
    expect(box).toBe('CQ TECQ ' + F2);
  });

  it('F2 poi F3: F3 sostituisce il resto di F2', async () => {
    const eng = fakeEngine();
    const seq = createLatestSequencer();
    await seq.run(() => eng.restart(F2)).done;
    eng.modulate(8);
    await seq.run(() => eng.restart('TU 73')).done;
    expect(eng.finish()).toBe('CQ TEST TU 73');
  });

  it('le pressioni rapide arrivano al motore in ordine e senza limite', async () => {
    const seq = createLatestSequencer();
    const order: number[] = [];
    const runs = Array.from({ length: 50 }, (_, i) =>
      seq.run(() => new Promise<number>((res) => setTimeout(() => (order.push(i), res(i)), (50 - i) % 7))),
    );
    await Promise.all(runs.map((r) => r.done));
    expect(order).toEqual(Array.from({ length: 50 }, (_, i) => i));
    expect(runs.filter((r) => seq.isLatest(r.gen)).length).toBe(1);
    expect(seq.isLatest(runs[49]!.gen)).toBe(true);
  });

  it('un errore non blocca le pressioni dopo; Esc rende vecchie tutte', async () => {
    const seq = createLatestSequencer();
    const a = seq.run(() => Promise.reject(new Error('busy')));
    const b = seq.run(() => Promise.resolve(2));
    await expect(a.done).rejects.toThrow('busy');
    await expect(b.done).resolves.toBe(2);
    seq.cancel();
    expect(seq.isLatest(b.gen)).toBe(false);
  });

  it('la finestra tiene solo i caratteri gia\' modulati', () => {
    expect(restartBox('CQ TEST IU3ABC', 4, 'TU')).toBe('CQ TTU');
    expect(restartBox('AB', 9, 'X')).toBe('ABX');
    expect(restartBox('AB', -1, 'X')).toBe('X');
  });
});

import { shortKey } from '../components/digi/ControlBar';
describe('tasto nel pulsante', () => {
  it('forma breve', () => {
    expect(shortKey('F1')).toBe('F1');
    expect(shortKey('Ctrl-F2')).toBe('C-F2');
    expect(shortKey('Shift-F12')).toBe('S-F12');
    expect(shortKey('Ctrl-Shift-F3')).toBe('CS-F3');
  });
});
