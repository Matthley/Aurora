import { describe, expect, it } from 'vitest';
import { clearDigiText, emitDigi, getDigiProvisional, getDigiText, getExtraProvisional, type DigiStreamEvent } from './digiBus';

const ev = (e: Record<string, unknown>) => ({ type: 'digi', ...e }) as unknown as DigiStreamEvent;

describe('CW veloce: testo provvisorio', () => {
  it('il provvisorio si sostituisce e non entra nel testo', () => {
    clearDigiText();
    emitDigi(ev({ kind: 'rx_prov', text: 'CQ TE' }));
    expect(getDigiProvisional()).toBe('CQ TE');
    emitDigi(ev({ kind: 'rx_text', text: 'CQ ' }));
    emitDigi(ev({ kind: 'rx_prov', text: 'TEST' }));
    expect(getDigiText()).toBe('CQ ');
    expect(getDigiProvisional()).toBe('TEST');
    clearDigiText();
    expect(getDigiProvisional()).toBe('');
  });

  it('quello di una finestra in piu\' resta suo', () => {
    emitDigi(ev({ kind: 'rx_prov', text: 'K1X', rx_id: 2 }));
    expect(getExtraProvisional(2)).toBe('K1X');
    expect(getDigiProvisional()).toBe('');
  });
});
