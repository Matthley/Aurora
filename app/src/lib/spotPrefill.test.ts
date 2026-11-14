import { describe, expect, it } from 'vitest';
import { spotBlocked, spotFreqKhz, spotPrefill } from './spotPrefill';

describe('spotPrefill', () => {
  it('somma il tono audio alla radio in USB', () => {
    expect(spotFreqKhz(14_110_000, 2_690, false, 2)).toBe('14112.69');
  });
  it('sottrae il tono audio in LSB', () => {
    expect(spotFreqKhz(7_045_000, 2_125, true, 2)).toBe('7042.88');
  });
  it('senza radio nota la frequenza resta vuota', () => {
    expect(spotFreqKhz(0, 1500)).toBe('');
  });
  it('prepara nominativo, frequenza e modo', () => {
    expect(spotPrefill(' iu3abc ', 14_080_000, 915, 'RTTY')).toEqual({ call: 'IU3ABC', khz: '14080.92', info: 'RTTY' });
    expect(spotPrefill('IU3ABC', 7_030_000, 700, 'CW', true).khz).toBe('7029.30');
  });
  it('blocca solo il telnet senza cluster collegato', () => {
    expect(spotBlocked('telnet', 0)).toBe(true);
    expect(spotBlocked('telnet', 1)).toBe(false);
    expect(spotBlocked('dxsummit', 0)).toBe(false);
    expect(spotBlocked(undefined, 0)).toBe(false);
  });
});
