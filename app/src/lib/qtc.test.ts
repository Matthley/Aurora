import { describe, expect, it } from 'vitest';
import { formatQtcLine, parseQtcHeader, parseQtcSeries, qtcHeader, qtcLineValid, qtcScripts } from './qtc';
import { runMacro } from './macroLang';

describe('QTC', () => {
  it('righe e intestazione da mandare', () => {
    expect(formatQtcLine({ ora: '1332', call: 'k5xyz', nr: '112' })).toBe('1332-K5XYZ-112');
    expect(qtcHeader(3, 7)).toBe('QTC 3/7 QTC 3/7');
  });

  it('gli script sono macro valide, senza comandi sconosciuti', () => {
    const rows = [{ ora: '1332', call: 'K5XYZ', nr: '112' }, { ora: '1340', call: 'W1AW', nr: '015' }];
    for (const s of [qtcScripts.header(1, 2), qtcScripts.line(rows[0]!), qtcScripts.all(1, rows), qtcScripts.qsl(), qtcScripts.qrv('iu3abc'), qtcScripts.agn(3), qtcScripts.agnAll(), qtcScripts.agnHeader(), qtcScripts.confirm('k1abc', 'iu3abc')]) {
      const ctx = { mycall: 'IU3ABC', myname: '', myqth: '', mywwloc: '', call: '', name: '', qth: '', wwloc: '', rstr: '', rsts: '', notes: '', nrr: '', nrs: '', startTime: null, now: new Date(), dialHz: 0, band: '', modeName: 'RTTY', qsoNumber: 1 };
      const r = runMacro(s, ctx);
      expect(r.actions.filter((a) => a.type === 'unknown')).toEqual([]);
      expect(r.actions[0]?.type).toBe('tx');
    }
    expect(qtcScripts.all(4, rows)).toContain('QTC 4/2 QTC 4/2<CRLF>1332-K5XYZ-112<CRLF>1340-W1AW-015<CRLF>QSL?');
  });

  it('intestazione ricevuta', () => {
    expect(parseQtcHeader('de K1ABC QTC 3/7 QTC 3/7')).toEqual({ serie: 3, n: 7 });
    expect(parseQtcHeader('qtc 12 / 10')).toEqual({ serie: 12, n: 10 });
    expect(parseQtcHeader('QTC 1/11')).toBeNull();
    expect(parseQtcHeader('niente')).toBeNull();
  });

  it('serie ricevuta dalla finestra RX', () => {
    const rx = 'CQ TEST\nK1ABC QTC 2/3 QTC 2/3\n1332-DL1ABC-112 1332-DL1ABC-112\n1340 F5XYZ 15\n» AGN 3\n1402/G4AAA/7 1402-G4AAA-007\nQSL?';
    const s = parseQtcSeries(rx);
    expect(s.serie).toBe(2);
    expect(s.n).toBe(3);
    expect(s.rows).toEqual([
      { ora: '1332', call: 'DL1ABC', nr: '112' },
      { ora: '1340', call: 'F5XYZ', nr: '015' },
      { ora: '1402', call: 'G4AAA', nr: '007' },
    ]);
  });

  it('senza intestazione prende le righe di tutto il testo', () => {
    const s = parseQtcSeries('0905-OK1ABC-099');
    expect(s.serie).toBeNull();
    expect(s.rows).toEqual([{ ora: '0905', call: 'OK1ABC', nr: '099' }]);
  });

  it('riga valida', () => {
    expect(qtcLineValid({ ora: '2359', call: 'K5XYZ', nr: '1' })).toBe(true);
    expect(qtcLineValid({ ora: '2460', call: 'K5XYZ', nr: '1' })).toBe(false);
    expect(qtcLineValid({ ora: '1200', call: 'ABC', nr: '1' })).toBe(false);
    expect(qtcLineValid({ ora: '1200', call: 'K5XYZ', nr: '' })).toBe(false);
  });
});
