import { describe, expect, it } from 'vitest';
import { cutText, pasteText } from './TxMenu';

describe('menu della finestra di trasmissione', () => {
  it('Taglia toglie la selezione, Incolla la sostituisce', () => {
    expect(cutText('CQ TEST IU3ABC', [3, 8])).toBe('CQ IU3ABC');
    expect(pasteText('CQ  IU3ABC', [3, 3], 'TEST')).toBe('CQ TEST IU3ABC');
    expect(pasteText('CQ XX IU3ABC', [3, 5], 'DE')).toBe('CQ DE IU3ABC');
  });
});
