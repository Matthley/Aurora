import { describe, expect, it } from 'vitest';
import { swapMacros } from './digiMacroEdit';
import { effectiveMacros } from './macroLang';

const m = (name: string, label: string) => ({ name, label, text: label, color: null });

describe('swapMacros', () => {
  it('scambia due macro di base', () => {
    const r = swapMacros({ mode: 'psk31' as const, macros: [m('F1', 'CQ'), m('F5', 'Bye')], mode_macros: {} }, 'F1', 'F5');
    expect(r.macros.find((x) => x.name === 'F5')?.label).toBe('CQ');
    expect(r.macros.find((x) => x.name === 'F1')?.label).toBe('Bye');
  });
  it('sposta su un tasto vuoto, anche in un\'altra riga', () => {
    const r = swapMacros({ mode: 'psk31' as const, macros: [m('F2', 'CQ')], mode_macros: {} }, 'F2', 'Ctrl-Shift-F12');
    expect(r.macros.map((x) => x.name)).toEqual(['Ctrl-Shift-F12']);
  });
  it('macro del modo e macro di base restano ciascuna nel suo insieme', () => {
    const s = { mode: 'rtty' as const, macros: [m('F1', 'base1'), m('F3', 'base3')], mode_macros: { rtty: [m('F1', 'rtty1')], cw: [m('F1', 'cw1')] } };
    const r = swapMacros(s, 'F1', 'F3');
    expect(r.mode_macros.rtty).toEqual([m('F3', 'rtty1')]);
    expect(r.mode_macros.cw).toEqual([m('F1', 'cw1')]);
    const eff = effectiveMacros(r.macros, r.mode_macros.rtty);
    expect(eff.get('F3')?.label).toBe('rtty1');
    expect(eff.get('F1')?.label).toBe('base3');
  });
  it('un tasto lasciato vuoto solo per il modo resta vuoto dopo lo scambio', () => {
    const s = { mode: 'cw' as const, macros: [m('F1', 'base1'), m('F2', 'base2')], mode_macros: { cw: [m('F2', '')] } };
    const r = swapMacros(s, 'F1', 'F2');
    const eff = effectiveMacros(r.macros, r.mode_macros.cw);
    expect(eff.get('F1')?.label).toBe('');
    expect(eff.get('F2')?.label).toBe('base1');
  });
  it('lo stesso tasto non cambia nulla', () => {
    const s = { mode: 'psk31' as const, macros: [m('F1', 'CQ')], mode_macros: {} };
    expect(swapMacros(s, 'F1', 'F1').macros).toEqual(s.macros);
  });
});
