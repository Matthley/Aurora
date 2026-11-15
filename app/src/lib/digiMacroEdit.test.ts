import { describe, expect, it } from 'vitest';
import { editMacros } from './digiMacroEdit';

const m = (name: string, label: string) => ({ name, label, text: label, color: null });

describe('editMacros nelle schede «Comuni» e «Solo per questo modo»', () => {
  const s = { mode: 'cw' as const, macros: [m('F2', 'CQ')], mode_macros: { cw: [m('F2', 'CQ CW')] } };
  it('la scheda Comuni cambia solo le comuni: la F2 di CW resta', () => {
    const r = editMacros(s, 'F2', m('F2', 'CQ nuovo'), 'all', true);
    expect(r.macros).toEqual([m('F2', 'CQ nuovo')]);
    expect(r.mode_macros.cw).toEqual([m('F2', 'CQ CW')]);
  });
  it('la scheda del modo cambia solo quelle del modo', () => {
    const r = editMacros(s, 'F2', m('F3', 'CW 3'), 'mode', true);
    expect(r.macros).toEqual([m('F2', 'CQ')]);
    expect(r.mode_macros.cw).toEqual([m('F3', 'CW 3')]);
  });
  it('dalla barra (senza schede) «tutti i modi» toglie ancora la versione del modo', () => {
    const r = editMacros(s, 'F2', m('F2', 'CQ nuovo'), 'all');
    expect(r.mode_macros.cw).toEqual([]);
  });
});
