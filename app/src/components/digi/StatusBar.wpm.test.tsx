import { describe, expect, it } from 'vitest';
import { CW_WPM_MAX, CW_WPM_MIN, stepCwWpm } from './StatusBar';

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
