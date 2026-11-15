import { describe, expect, it } from 'vitest';
import { foxCalls, foxSender, lineSender } from './fox';

describe('stazioni Fox/Hound', () => {
  it('riconosce le righe del Fox di WSJT-X e non le risposte multiple di MSHV', () => {
    expect(foxSender('K1ABC RR73; W9XYZ <KH1/KH7Z> -08')).toBe('KH1/KH7Z');
    expect(foxSender('IU3WJQ V26JJ R+03')).toBeNull();
    expect(foxSender('R2PA V26JJ RR73')).toBeNull();
    const fox = foxCalls(['K1ABC RR73; W9XYZ <KH1/KH7Z> -08', 'R2PA V26JJ RR73', 'R7RDE V26JJ -04']);
    expect([...fox]).toEqual(['KH1/KH7Z']);
    expect(lineSender('IU3ABC KH1/KH7Z -12')).toBe('KH1/KH7Z');
    expect(lineSender('CQ DX V26JJ FK97')).toBe('V26JJ');
  });

  it('ignores the DXpedition message of a normal station above 1000 Hz', () => {
    const m = 'SP6PST RR73; K1ABC <G3VCQ> -07';
    expect(foxCalls([{ text: m, freq_hz: 1650 }]).has('G3VCQ')).toBe(false);
    expect(foxCalls([{ text: m, freq_hz: 600 }]).has('G3VCQ')).toBe(true);
  });

  it('MSHV multi-stream in normal mode (issue #68) is not a Fox', () => {
    // 3X4U con MSHV: sei flussi nello stesso periodo, risposte normali e righe combinate, chiamato anche sotto i 1000 Hz
    const items = [
      { text: 'SP2MWZ 3X4U -12', freq_hz: 517 },
      { text: 'G3SBP 3X4U +06', freq_hz: 577 },
      { text: 'IU3ABC 3X4U -09', freq_hz: 637 },
      { text: 'K1ABC RR73; W9XYZ <3X4U> -08', freq_hz: 697 },
      { text: 'DL1ABC 3X4U RR73', freq_hz: 757 },
      { text: 'F5ABC 3X4U -15', freq_hz: 817 },
      { text: '3X4U OH5BM KP41', freq_hz: 640 },
    ];
    expect(foxCalls(items).has('3X4U')).toBe(false);
    // le righe di un flusso sopra i 1000 Hz bastano anche senza chiamanti bassi
    const high = [
      { text: 'K1ABC RR73; W9XYZ <3X4U> -08', freq_hz: 517 },
      { text: 'SP2MWZ 3X4U -12', freq_hz: 1517 },
      { text: '3X4U OH5BM KP41', freq_hz: 2266 },
    ];
    expect(foxCalls(high).has('3X4U')).toBe(false);
  });

  it('a WSJT-X Fox with Hounds above 1000 Hz is a Fox', () => {
    const items = [
      { text: 'K1ABC RR73; W9XYZ <KH1/KH7Z> -08', freq_hz: 300 },
      { text: 'N2BJ RR73; DL1ABC <KH1/KH7Z> -11', freq_hz: 360 },
      { text: 'IU3ABC KH1/KH7Z -12', freq_hz: 420 },
      { text: 'KH1/KH7Z W9XYZ R-05', freq_hz: 1450 },
      { text: 'KH1/KH7Z OH5BM KP41', freq_hz: 2100 },
    ];
    expect(foxCalls(items).has('KH1/KH7Z')).toBe(true);
  });

  it('a station that sends R+report is never a Fox', () => {
    const items = [{ text: 'SP6PST RR73; K1ABC <N2BJ> -07', freq_hz: 600 }, { text: 'DG7RF N2BJ R-12', freq_hz: 1450 }];
    expect(foxCalls(items).has('N2BJ')).toBe(false);
  });
});
