import { describe, expect, it } from 'vitest';
import { newTextContest, type Domain, type TextContest } from '../api/contestTesto';
import { categoryValid, contestDateMs, contestQsos, contestWindow, domainLookup, grabExchange, modeCategory, startSerial, textContestStatus, withContestMacros } from './textContest';

const c = (p: Partial<TextContest> = {}): TextContest => ({ ...newTextContest(), name: 'Prova', ...p });
const now = Date.UTC(2026, 8, 26, 12, 0, 0);

describe('finestra del contest', () => {
  it('legge le date e le porta nell anno di adesso', () => {
    expect(contestDateMs('20150926000001')).toBe(Date.UTC(2015, 8, 26, 0, 0, 1));
    expect(contestDateMs('Invalid')).toBeNull();
    const w = contestWindow(c({ start: '20150926000001', end: '20150927235959' }), now);
    expect(w).toEqual({ from: Date.UTC(2026, 8, 26, 0, 0, 1), to: Date.UTC(2026, 8, 27, 23, 59, 59) });
    expect(contestWindow(c(), now)).toBeNull();
  });

  it('tiene solo i QSO dentro la finestra e delle categorie valide', () => {
    const qsos = [
      { call: 'IU3ABC', band: '20m', mode: 'RTTY', date: '2026-09-26', time: '10:00' },
      { call: 'IU3XYZ', band: '20m', mode: 'RTTY', date: '2026-09-20', time: '10:00' },
      { call: 'IU3CW', band: '20m', mode: 'CW', date: '2026-09-26', time: '11:00' },
    ];
    const k = c({ start: '20150926000001', end: '20150927235959', valid_digital: true });
    expect(contestQsos(qsos, k, now).map((q) => q.call)).toEqual(['IU3ABC']);
    expect(contestQsos(qsos, c({ valid_digital: false }), now)).toHaveLength(3);
  });
});

describe('categorie e doppi', () => {
  it('classifica i modi', () => {
    expect(modeCategory('CW')).toBe('cw');
    expect(modeCategory('ssb')).toBe('phone');
    expect(modeCategory('PSK31')).toBe('digital');
    expect(categoryValid(c({ valid_digital: false }), 'RTTY')).toBe(true);
    expect(categoryValid(c({ valid_cw: true, valid_digital: false }), 'RTTY')).toBe(false);
  });

  it('doppio con e senza banda o modo', () => {
    const qsos = [{ call: 'IU3ABC', band: '20m', mode: 'RTTY', date: '2026-09-26', time: '10:00' }];
    expect(textContestStatus(qsos, c(), 'iu3abc', '20m', 'RTTY')).toBe('dupe');
    expect(textContestStatus(qsos, c(), 'IU3ABC', '40m', 'RTTY')).toBe('new_band');
    expect(textContestStatus(qsos, c({ dupe_no_band: true }), 'IU3ABC', '40m', 'RTTY')).toBe('dupe');
    expect(textContestStatus(qsos, c(), 'IU3ABC', '20m', 'PSK31')).toBe('new_mode');
    expect(textContestStatus(qsos, c({ dupe_no_mode: true }), 'IU3ABC', '20m', 'PSK31')).toBe('dupe');
    expect(textContestStatus(qsos, c(), 'IU3XYZ', '20m', 'RTTY')).toBe('new_call');
  });
});

describe('macro, numero e cattura', () => {
  const m = (name: string, label: string) => ({ name, label, text: '', color: null });
  it('unisce le macro del contest', () => {
    const eff = new Map([['F1', m('F1', 'CQ')], ['F2', m('F2', 'DE')]]);
    expect([...withContestMacros(eff, [m('F1', 'CQ TEST')], true).values()].map((x) => x.label)).toEqual(['CQ TEST', 'DE']);
    expect([...withContestMacros(eff, [m('F1', 'CQ TEST')], false).values()].map((x) => x.label)).toEqual(['CQ TEST']);
    expect(withContestMacros(eff, [], false).size).toBe(2);
  });

  it('primo numero', () => {
    expect(startSerial('')).toBe(1);
    expect(startSerial('015')).toBe(15);
    expect(startSerial('14')).toBe(14);
  });

  const dom: Domain = { title: '', country: 'I', entries: [{ key: 'TO', value: 'TO', section: 'I1' }, { key: 'MILANO', value: 'MI', section: '' }, { key: 'TV', value: 'TV', section: '' }] };
  it('dominio e numero dal testo ricevuto', () => {
    expect(domainLookup(dom, 'milano', 'IU2ABC')).toBe('MI');
    expect(grabExchange('IU3XYZ DE IU3ABC 599 TV TV K', 'IU3ABC', { number: true, domain: dom })).toBe('TV');
    expect(grabExchange('CQ IU3XYZ de IU3ABC 599 014 014 K', 'IU3ABC', { number: true, domain: null })).toBe('014');
    expect(grabExchange('IU3ABC 599 K', 'IU3ABC', { number: true, domain: null })).toBeNull();
    expect(grabExchange('nessun nominativo 014', 'IU3ABC', { number: true, domain: null })).toBeNull();
    expect(grabExchange('IU3ABC 599 014', 'IU3ABC', { number: false, domain: null })).toBeNull();
  });
});
