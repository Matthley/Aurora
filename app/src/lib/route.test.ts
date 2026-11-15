import { describe, expect, it } from 'vitest';
import { isSettingsRoute, parseRoute, remoteFallback } from './route';

describe('parseRoute', () => {
  it('reads the pairing link with the code', () => {
    expect(parseRoute('#/pair/12345678')).toEqual({ name: 'pair', code: '12345678' });
  });

  it('reads the pairing page without a code', () => {
    expect(parseRoute('#/pair')).toEqual({ name: 'pair', code: '' });
  });

  it('keeps only digits of the code', () => {
    expect(parseRoute('#/pair/12ab34')).toEqual({ name: 'pair', code: '1234' });
  });

  it('reads the settings and automation pages', () => {
    expect(parseRoute('#/impostazioni')).toEqual({ name: 'settings' });
    expect(parseRoute('#/automazioni')).toEqual({ name: 'settings', tab: 'automation' });
  });

  it('opens a precise settings tab', () => {
    expect(parseRoute('#/impostazioni/cluster')).toEqual({ name: 'settings', tab: 'cluster' });
    expect(parseRoute('#/impostazioni/')).toEqual({ name: 'live' });
  });

  it('reads the statistics and guide pages', () => {
    expect(parseRoute('#/esporta')).toEqual({ name: 'export' });
    expect(parseRoute('#/statistiche')).toEqual({ name: 'stats' });
    expect(parseRoute('#/guida')).toEqual({ name: 'guide' });
  });

  it('reads the map and history pages', () => {
    // la vecchia Mappa ora porta alla Mappa DX
    expect(parseRoute('#/mappa')).toEqual({ name: 'dxmap' });
    expect(parseRoute('#/storico')).toEqual({ name: 'history' });
    expect(parseRoute('#/propagazione')).toEqual({ name: 'propagation' });
  });

  it('defaults to the live page', () => {
    expect(parseRoute('')).toEqual({ name: 'live' });
    expect(parseRoute('#/')).toEqual({ name: 'live' });
    expect(parseRoute('#/sconosciuta')).toEqual({ name: 'live' });
  });
});

describe('remoteFallback', () => {
  it('sends every Settings route of a remote client back to the main page', () => {
    for (const hash of ['#/impostazioni', '#/impostazioni/clock', '#/automazioni', '#/finestra/impostazioni']) {
      expect(remoteFallback(parseRoute(hash), true)).toEqual({ name: 'live' });
      expect(isSettingsRoute(parseRoute(hash))).toBe(true);
    }
  });
  it('leaves the Settings open on the PC and the other pages untouched on a remote client', () => {
    expect(remoteFallback(parseRoute('#/impostazioni/clock'), false)).toEqual({ name: 'settings', tab: 'clock' });
    expect(remoteFallback(parseRoute('#/statistiche'), true)).toEqual({ name: 'stats' });
    expect(remoteFallback(parseRoute('#/finestra/mappa'), true)).toEqual({ name: 'window', id: 'mappa' });
  });
});
