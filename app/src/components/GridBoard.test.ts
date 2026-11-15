import { describe, expect, it } from 'vitest';
import { swapBand, swapCells, swapUnit } from './GridBoard';

// Stazione FT8 con RX acceso: RX sopra il Log, a destra Bande aperte a tutta altezza
const cells = {
  rx: { x: 0, y: 0, w: 6, h: 5 },
  log: { x: 0, y: 5, w: 6, h: 9 },
  dxmap: { x: 6, y: 0, w: 6, h: 14 },
};
const linked = { rx: 'log', log: 'rx' };

describe('scambio dei blocchi legati', () => {
  it('senza legame RX non ha vicini con cui scambiarsi (il difetto)', () => {
    expect(swapBand(cells, 'rx')).toHaveLength(1);
  });

  it('RX e Log formano una sola unita alta quanto il vicino', () => {
    expect(swapUnit(cells, 'rx', linked)).toEqual({ ids: ['rx', 'log'], cell: { x: 0, y: 0, w: 6, h: 14 } });
    expect(swapBand(cells, 'log', linked).map((u) => u.ids)).toEqual([['rx', 'log'], ['dxmap']]);
  });

  it('trascinando la barretta di RX si spostano insieme RX e Log', () => {
    const [own, other] = swapBand(cells, 'rx', linked);
    const next = swapCells(cells, own!, other!);
    expect(next.rx).toEqual({ x: 6, y: 0, w: 6, h: 5 });
    expect(next.log).toEqual({ x: 6, y: 5, w: 6, h: 9 });
    expect(next.dxmap).toEqual({ x: 0, y: 0, w: 6, h: 14 });
  });
});
