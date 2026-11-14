import { describe, expect, it } from 'vitest';
import { TXE_TIMEOUT_MS, txeStep } from './txeWait';

describe('<TXE>', () => {
  it('trasmette subito senza segnale', () => {
    expect(txeStep(false, 0, 0)).toBe('send');
    expect(txeStep(undefined, 0, 0)).toBe('send');
  });
  it('aspetta finche il segnale c e', () => {
    expect(txeStep(true, 1000, 1000 + TXE_TIMEOUT_MS - 1)).toBe('wait');
  });
  it('passato il tempo massimo trasmette comunque', () => {
    expect(txeStep(true, 1000, 1000 + TXE_TIMEOUT_MS)).toBe('send');
    expect(txeStep(true, 0, 500, 500)).toBe('send');
  });
});
