import { describe, expect, it } from 'vitest';
import { CW_WPM_MAX, CW_WPM_MIN, followRxPatch, followRxTarget, stepCwWpm, wpmStepPatch } from './StatusBar';

describe('stepCwWpm', () => {
  it('passo 1, con Maiuscolo 5', () => {
    expect(stepCwWpm(20, 1, false)).toBe(21);
    expect(stepCwWpm(20, -1, false)).toBe(19);
    expect(stepCwWpm(20, 1, true)).toBe(25);
    expect(stepCwWpm(20, -1, true)).toBe(15);
  });
  it('resta entro minimo e massimo', () => {
    expect(stepCwWpm(CW_WPM_MIN + 2, -1, true)).toBe(CW_WPM_MIN);
    expect(stepCwWpm(CW_WPM_MAX - 1, 1, true)).toBe(CW_WPM_MAX);
  });
});

describe('interruttore TX = RX', () => {
  it('acceso, la velocita di trasmissione segue la misura arrotondata, entro 5 e 60', () => {
    expect(followRxTarget(true, 23.4, 20)).toBe(23);
    expect(followRxTarget(true, 3, 20)).toBe(CW_WPM_MIN);
    expect(followRxTarget(true, 90, 20)).toBe(CW_WPM_MAX);
  });
  it('non fa nulla se la velocita e gia quella, se la misura manca o se e spento', () => {
    expect(followRxTarget(true, 23.4, 23)).toBeNull();
    expect(followRxTarget(true, null, 20)).toBeNull();
    expect(followRxTarget(false, 23, 20)).toBeNull();
    expect(followRxTarget(undefined, 23, 20)).toBeNull();
  });
  it('accenderlo salva subito la misura; senza misura accende solo l interruttore; spegnerlo non tocca la velocita', () => {
    expect(followRxPatch(true, 25.6)).toEqual({ cw_wpm_follow_rx: true, cw_wpm: 26 });
    expect(followRxPatch(true, null)).toEqual({ cw_wpm_follow_rx: true });
    expect(followRxPatch(false, 25)).toEqual({ cw_wpm_follow_rx: false });
  });
  it('i tasti - e + salvano la velocita e lo spengono', () => {
    expect(wpmStepPatch(stepCwWpm(20, 1, false))).toEqual({ cw_wpm: 21, cw_wpm_follow_rx: false });
    expect(wpmStepPatch(stepCwWpm(20, -1, true))).toEqual({ cw_wpm: 15, cw_wpm_follow_rx: false });
  });
});
