import { describe, expect, it } from 'vitest';
import { HELP_CATEGORIES, MACRO_COMMANDS, MACRO_EVENTS, UNMAPPED } from './macroCommands';
import { EVENT_MACROS, runMacro, type MacroAction, type MacroContext } from './macroLang';
import { planMacro, splitAutoCq } from './macroPlan';

const ctx: MacroContext = {
  mycall: 'iu3abc',
  myname: 'Gianni',
  myqth: 'Verona',
  mywwloc: 'io91wm',
  call: 'dl1xyz',
  name: 'Hans',
  qth: 'Berlin',
  wwloc: 'jo62',
  rstr: '579',
  rsts: '599',
  notes: '',
  nrr: '',
  nrs: '007',
  startTime: null,
  now: new Date(Date.UTC(2026, 10, 14, 14, 5)),
  dialHz: 14_070_000,
  band: '20m',
  modeName: 'RTTY',
  qsoNumber: 7,
};

const plan = (script: string, sending = false) => planMacro(runMacro(script, ctx).steps, sending);

describe('planMacro: <TX> e <RX> nel testo', () => {
  it('CQ con <TX> ... <RX>: parte il testo, poi ricezione', () => {
    const p = plan('<TX>CQ CQ DE <MYCALL> K<RX>');
    expect(p).toMatchObject({ tx: 'start', text: 'CQ CQ DE IU3ABC K', after: '', rxAfter: true, clearAfter: false, clearBox: false });
  });

  it('il testo dopo <RX> non parte: resta nella finestra', () => {
    const p = plan('<TX>73 <CALL><RX>QRZ?');
    expect(p.text).toBe('73 DL1XYZ');
    expect(p.after).toBe('QRZ?');
    expect(p.rxAfter).toBe(true);
  });

  it('<RXANDCLEAR> e <RXC>: ricezione e pulizia a fine trasmissione', () => {
    expect(plan('<TX>TU<RXANDCLEAR>')).toMatchObject({ tx: 'start', rxAfter: true, clearAfter: true, clearBox: false });
    expect(plan('<TX>TU<RXC>')).toMatchObject({ rxAfter: true, clearAfter: true });
  });

  it('<RXE>: ricezione a fine testo, il testo dopo parte lo stesso', () => {
    expect(plan('<TX><RXE>QRZ? DE <MYCALL>')).toMatchObject({ text: 'QRZ? DE IU3ABC', after: '', rxAfter: true });
  });

  it('senza trasmissione <RX> non ha niente da chiudere: tutto il testo va nella casella', () => {
    expect(plan('A<RX>B')).toMatchObject({ tx: 'none', text: 'AB', after: '', rxAfter: false });
    // <RXC> senza trasmissione pulisce subito quello che c'era prima
    expect(plan('A<RXC>B')).toMatchObject({ clearBox: true, text: 'B' });
  });

  it('<RX> durante una trasmissione aperta: il testo si accoda, poi ricezione', () => {
    expect(plan('TU 73<RX>', true)).toMatchObject({ tx: 'none', text: 'TU 73', rxAfter: true });
    expect(plan('<RX>', true)).toMatchObject({ text: '', rxAfter: true });
  });

  it('<TXE>: aspetta che cada il segnale', () => {
    expect(plan('<TXE>DE <MYCALL><RX>')).toMatchObject({ tx: 'start', waitCarrier: true, rxAfter: true });
  });

  it('<TXTOGGLE>: apre in ricezione, chiude in trasmissione', () => {
    expect(plan('<TXTOGGLE>').tx).toBe('start');
    expect(plan('<TXTOGGLE>', true).tx).toBe('stop');
  });
});

describe('planMacro: pulizia della finestra di trasmissione secondo la posizione', () => {
  it('<CLEARTXWINDOW> prima del testo: si pulisce e poi entra il testo', () => {
    expect(plan('<CLEARTXWINDOW>CQ DE <MYCALL>')).toMatchObject({ clearBox: true, text: 'CQ DE IU3ABC' });
  });

  it('<CLEARTXWINDOW> dopo il testo: lo cancella', () => {
    expect(plan('CQ DE <MYCALL><CLEARTXWINDOW>')).toMatchObject({ clearBox: true, text: '' });
  });

  it('<CLEARTXWINDOW> dopo <RX>: pulisce tornati in ricezione', () => {
    expect(plan('<TX>TU<RX><CLEARTXWINDOW>')).toMatchObject({ text: 'TU', clearAfter: true, clearBox: false });
  });

  it('<CLEARWINDOW>: pulisce anche la ricezione', () => {
    const p = plan('<CLEARWINDOW>');
    expect(p.clearBox).toBe(true);
    expect(p.actions).toEqual([{ type: 'clearRx' }]);
  });
});

describe('planMacro: gli altri comandi subito, nell\'ordine', () => {
  it('<SAVEQSO> prima di <CLEARQSO>', () => {
    const p = plan('<TX>TU <MYCALL><RX><SAVEQSO><CLEARQSO>');
    expect(p.actions).toEqual([{ type: 'saveQso' }, { type: 'clearQso' }]);
    expect(p.text).toBe('TU IU3ABC');
  });

  it('modo, rotore e frequenza restano nell\'ordine dello script', () => {
    expect(plan('<MODE:RTTY><ROTOR:SP><FQ:14080>').actions.map((a) => a.type)).toEqual(['mode', 'rotor', 'fq']);
  });
});

describe('splitAutoCq', () => {
  it('<AUTOCQ> seguito da testo: il testo e\' la chiamata', () => {
    expect(splitAutoCq('<AUTOCQ>CQ CQ DE <MYCALL> K<RX>')).toEqual({ before: '', cq: 'CQ CQ DE <MYCALL> K<RX>' });
    expect(splitAutoCq('<CLEARTXWINDOW><ASAUTOCQ>CQ TEST<RX>')).toEqual({ before: '<CLEARTXWINDOW>', cq: 'CQ TEST<RX>' });
  });
  it('<AUTOCQ> da solo resta un comando', () => {
    expect(splitAutoCq('<AUTOCQ>')).toEqual({ before: '<AUTOCQ>', cq: null });
    expect(splitAutoCq('CQ<RX>')).toEqual({ before: 'CQ<RX>', cq: null });
  });
});

describe('runMacro: steps nell\'ordine dello script', () => {
  it('testo e comandi alternati', () => {
    expect(runMacro('A<TX>B<CALL>', ctx).steps).toEqual([{ text: 'A' }, { action: { type: 'tx' } }, { text: 'B' }, { text: 'DL1XYZ' }]);
  });
});

// l'effetto atteso di ogni comando di programma (esempio -> tipo di azione)
const EFFECT: Record<string, MacroAction['type']> = {
  '<TXTOGGLE>': 'txToggle', '<CLEARRXWINDOW>': 'clearRx', '<CLEARWINDOW>': 'clearWindow', '<CURSOR:TX>': 'cursor', '<BEEP:ON>': 'beep',
  '<ALARM:QSO saved>': 'alarm', '<AUDIOFQ:1500>': 'audioFq', '<SEEKLEFT>': 'seek', '<SEEKRIGHT>': 'seek', '<FQ:14070.5>': 'fq', '<SPLIT:1>': 'toggle',
  '<SPLIT:0>': 'toggle', '<JUMPTOTXFQ>': 'jumpToTx', '<ALIGN:50>': 'align', '<STARTSCAN>': 'scan', '<STOPSCAN>': 'scan', '<MODE:RTTY>': 'mode',
  '<BAUDRATE:45.45>': 'baud', '<SHIFT:170>': 'shift', '<AFCON>': 'toggle', '<AFCOFF>': 'toggle', '<AFC>': 'toggle', '<SQUELCH>': 'toggle',
  '<SQUELCHON>': 'toggle', '<SQUELCHOFF>': 'toggle', '<SNAPON>': 'toggle', '<SNAPOFF>': 'toggle', '<SNAP>': 'toggle', '<LOCKTX>': 'toggle',
  '<UNLOCKTX>': 'toggle', '<LOCKTXTOGGLE>': 'toggle', '<MODESETTINGS>': 'modeSettings', '<THRESHOLD>': 'threshold', '<MEMW:1>': 'memory',
  '<MEMR:1>': 'memory', '<ACQ>': 'autoCq', '<AUTOCQ>': 'autoCq', '<ACQDELAY:15>': 'acqDelay', '<ASAUTOCQ>': 'autoCq', '<AUTOCQRESUME>': 'autoCq',
  '<ROTOR:45>': 'rotor', '<ROTOR:SP>': 'rotor', '<ROTOR:LP>': 'rotor', '<CATCMD:AN1;>': 'cat', '<EDITMACRO:F5>': 'editMacro', '<WPM:+2>': 'wpm',
  '<IGNOREBEFORE:2026-11-14 12:00>': 'ignoreBefore', '<PERIOD:60>': 'period', '<CLEARQSO>': 'clearQso', '<FIXSTARTTIME>': 'fixStartTime',
  '<SAVEQSO>': 'saveQso', '<TX>': 'tx', '<TXE>': 'txe', '<RX>': 'rx', '<RXANDCLEAR>': 'rx', '<RXC>': 'rx', '<RXE>': 'rxe', '<CLEARTXWINDOW>': 'clearTx',
  // comandi che la pagina riconosce e per cui dice che non li sa fare
  '<TUNE:5>': 'tune', '<BOOKMARK>': 'bookmark', '<MACROS:contest.mc>': 'macros',
};

describe('ogni comando ha il suo effetto', () => {
  it.each(Object.entries(EFFECT))('%s -> %s', (script, type) => {
    const r = runMacro(script, ctx);
    expect(r.actions.map((a) => a.type)).toEqual([type]);
  });

  it('le azioni con i loro valori', () => {
    expect(runMacro('<ACQDELAY:15>', ctx).actions).toEqual([{ type: 'acqDelay', seconds: 15 }]);
    expect(runMacro('<ALIGN:50>', ctx).actions).toEqual([{ type: 'align', hz: 50 }]);
    expect(runMacro('<AUTOCQ><ASAUTOCQ><AUTOCQRESUME>', ctx).actions).toEqual([
      { type: 'autoCq', how: 'toggle' },
      { type: 'autoCq', how: 'start' },
      { type: 'autoCq', how: 'resume' },
    ]);
    expect(runMacro('<ALIGN:0>', ctx).actions).toEqual([{ type: 'unknown', tag: 'ALIGN:0' }]);
  });
});

describe('l\'elenco dei comandi (unica fonte per editor e Guida)', () => {
  it('ogni voce ha categoria ed esempio', () => {
    expect(UNMAPPED).toEqual([]);
    for (const c of MACRO_COMMANDS) {
      expect(HELP_CATEGORIES).toContain(c.cat);
      expect(c.ex.trim(), c.tag).not.toBe('');
      for (const d of [...c.desc, ...(c.long ?? [])]) expect(d.trim(), c.tag).not.toBe('');
    }
  });

  it('gli esempi dei comandi disponibili non danno comandi sconosciuti', () => {
    for (const c of MACRO_COMMANDS.filter((x) => !x.soon)) {
      const r = runMacro(c.ex, ctx);
      expect(r.actions.filter((a) => a.type === 'unknown'), c.tag).toEqual([]);
    }
  });

  it('i comandi «non ancora» non fanno niente (sconosciuti, rifiutati o lasciati nel testo)', () => {
    for (const c of MACRO_COMMANDS.filter((x) => x.soon)) {
      const r = runMacro(c.ex, ctx);
      const done = r.actions.filter((a) => !['unknown', 'tune', 'bookmark', 'macros'].includes(a.type));
      expect(done, c.tag).toEqual([]);
    }
  });

  it('ogni comando che il parser capisce e\' nell\'elenco (come voce o come altro nome)', () => {
    const listed = new Set(MACRO_COMMANDS.flatMap((c) => [c.tag, ...(c.aliases ?? [])]).map((t) => t.replace(/:.*>$/, '>')).flatMap((t) => t.split(' ')));
    const parsed = ['MYCALL', 'MYMAINCALL', 'MYNAME', 'MYQTH', 'MYWWLOC', 'CALL', 'NAME', 'QTH', 'WWLOC', 'RSTR', 'RSTS', 'NOTES', 'NRR', 'NRS', 'DATE', 'TIME',
      'LOCALTIME', 'STARTTIME', 'CTIME', 'CR', 'CRLF', 'LF', 'TAB', '[', ']', 'GA', 'GAL', 'KHZ', 'MHZ', 'BAND', 'MODE', 'QSONR', 'VER', 'ANYQSOBEFOREDATE',
      'ANYQSOBEFOREBAND', 'ANYQSOBEFOREMODE', 'TEXTFILE', 'IMD', 'AR', 'BT', 'KN', 'SK', 'TX', 'TXE', 'TXTOGGLE', 'RX', 'RXANDCLEAR', 'RXC', 'RXE',
      'CLEARTXWINDOW', 'CLEARRXWINDOW', 'CLEARQSO', 'CLEARWINDOW', 'FIXSTARTTIME', 'SAVEQSO', 'AUTOCQ', 'ACQ', 'ASAUTOCQ', 'AUTOCQRESUME', 'ACQDELAY',
      'JUMPTOTXFQ', 'ALIGN', 'BEEP', 'SEEKLEFT', 'SEEKRIGHT', 'BOOKMARK', 'AFCON', 'AFCOFF', 'AFC', 'SNAPON', 'SNAPOFF', 'SNAP', 'SQUELCHON',
      'SQUELCHOFF', 'SQUELCH', 'LOCKTX', 'UNLOCKTX', 'LOCKTXTOGGLE', 'STARTSCAN', 'STOPSCAN', 'MODESETTINGS', 'IGNOREBEFORE', 'PERIOD', 'THRESHOLD',
      'MEMW', 'MEMR', 'TUNE', 'CURSOR', 'MACROS', 'CATCMD', 'BAUDRATE', 'SHIFT', 'FQ', 'SPLIT', 'ROTOR', 'EDITMACRO', 'ALARM', 'WPM', 'AUDIOFQ'];
    for (const tag of parsed) expect(listed.has(`<${tag}>`), tag).toBe(true);
  });

  it('le macro evento della Guida sono quelle dell\'editor', () => {
    expect(MACRO_EVENTS.map((e) => e.name).sort()).toEqual([...EVENT_MACROS].sort());
  });
});
