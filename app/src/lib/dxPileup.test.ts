import { describe, expect, it } from 'vitest';
import type { DigiSignal } from '../api/schemas';
import { estimate, newText, PileupTracker, splitHint } from './dxPileup';

const sig = (id: number, hz: number, text: string, calls: DigiSignal['calls'] = []): DigiSignal => ({ id, freq_hz: hz, snr_db: 10, text, unsure: [], calls, cq: false }) as DigiSignal;
const dxCall = (role = 'cq') => [{ call: 'EA8DX', role, verified: true }];

describe('modo DXpedition', () => {
  it('il testo nuovo di un canale anche quando la coda scorre', () => {
    expect(newText('', 'CQ')).toBe('CQ');
    expect(newText('CQ DE', 'CQ DE EA8DX')).toBe(' EA8DX');
    expect(newText('ABCDEFGH', 'EFGHIJ')).toBe('IJ');
    expect(newText('CQ', 'CQ')).toBe('');
  });

  it('la DX a 1000 Hz manda K1XYZ 5NN, K1XYZ sentito a 1350 Hz 3 s prima: ascolta a 1350', () => {
    const t = new PileupTracker();
    t.setDx('EA8DX');
    t.update([sig(1, 1000, 'CQ EA8DX UP ', dxCall()), sig(2, 1350, '')], 0);
    t.update([sig(1, 1000, 'CQ EA8DX UP ', dxCall()), sig(2, 1350, 'K1XYZ K1XYZ ')], 7000);
    t.update([sig(1, 1000, 'CQ EA8DX UP K1XYZ 5NN ', dxCall()), sig(2, 1350, 'K1XYZ K1XYZ ')], 10_000);
    const st = t.state();
    expect(st.worked.map((w) => [w.call, w.hz])).toEqual([['K1XYZ', 1350]]);
    expect(st.estimateHz).toBe(1350);
    expect(st.dxHz).toBe(1000);
  });

  it('un nominativo sentito oltre 60 s prima, o in simplex, non conta', () => {
    const t = new PileupTracker();
    t.setDx('EA8DX');
    t.update([sig(1, 1000, 'X ', dxCall()), sig(2, 1350, ''), sig(3, 1010, '')], 0);
    t.update([sig(1, 1000, 'X ', dxCall()), sig(2, 1350, 'K1XYZ '), sig(3, 1010, 'DL1AB ')], 1000);
    t.update([sig(1, 1000, 'X K1XYZ 599 DL1AB TU ', dxCall()), sig(2, 1350, 'K1XYZ '), sig(3, 1010, 'DL1AB ')], 70_000);
    expect(t.state().worked).toEqual([]);
  });

  it('i chiamanti salgono: la stima segue lo spostamento', () => {
    const t = new PileupTracker();
    t.setDx('EA8DX');
    const callers = ['K1AA', 'K2BB', 'K3CC', 'K4DD'];
    let dx = 'CQ EA8DX ';
    let now = 0;
    t.update([sig(1, 1000, dx, dxCall())], now);
    callers.forEach((c, i) => {
      now += 5000;
      t.update([sig(1, 1000, dx, dxCall()), sig(10 + i, 2000 + i * 200, `${c} ${c} `)], now);
      now += 3000;
      dx += `${c} 5NN TU `;
      t.update([sig(1, 1000, dx, dxCall()), sig(10 + i, 2000 + i * 200, `${c} ${c} `)], now);
    });
    const st = t.state();
    expect(st.worked.map((w) => w.hz)).toEqual([2600, 2400, 2200, 2000]);
    expect(st.drift).toBe('up');
    expect(st.estimateHz).toBe(2800);
    expect(st.windowHz).toEqual([2000, 2600]);
  });

  it('stima ferma se i passi non vanno tutti nello stesso verso', () => {
    const w = (hz: number[]) => hz.map((h, i) => ({ call: `K${i}A`, hz: h, at: i }));
    expect(estimate(w([1500, 1700, 1400])).estimateHz).toBe(1500);
    expect(estimate(w([1500, 1700, 1400])).drift).toBe('none');
    expect(estimate([]).estimateHz).toBeNull();
  });

  it('UP 2, UP 1-3, DOWN e UP da solo in kHz (1 kHz = 1000 Hz di tono)', () => {
    expect(splitHint('CQ EA8DX UP 2')).toBe(2000);
    expect(splitHint('EA8DX UP2')).toBe(2000);
    expect(splitHint('UP 1-3 TU')).toBe(2000);
    expect(splitHint('QRZ DOWN 1')).toBe(-1000);
    expect(splitHint('TU UP')).toBe(1000);
    expect(splitHint('K1XYZ 599')).toBeNull();
  });

  it('senza chiamanti trovati vale l indicazione UP dalla frequenza della DX', () => {
    const t = new PileupTracker();
    t.setDx('EA8DX');
    t.update([sig(1, 800, '', dxCall())], 0);
    t.update([sig(1, 800, 'CQ EA8DX UP 2 ', dxCall())], 1000);
    const st = t.state();
    expect(st.estimateHz).toBeNull();
    expect(st.hintHz).toBe(2800);
  });
});
