/**
 * Lo spot di una stazione sentita: la frequenza vera e' quella della radio piu' il tono audio della stazione (meno, se la radio e' in LSB),
 * in kHz; il commento parte dal modo (RTTY, CW, PSK31...).
 */
export function spotFreqKhz(dialHz: number, audioHz: number, lsb = false, decimals = 1): string {
  if (!(dialHz > 0)) return '';
  const hz = lsb ? dialHz - audioHz : dialHz + audioHz;
  return hz > 0 ? (hz / 1000).toFixed(decimals) : '';
}

export interface SpotPrefill {
  call: string;
  khz: string;
  info: string;
}

/** I campi gia' pronti della finestra «Invia spot» per una stazione dell'elenco: nominativo, frequenza (al centesimo di kHz) e modo. */
export function spotPrefill(call: string, dialHz: number, audioHz: number, modeName: string, lsb = false): SpotPrefill {
  return { call: call.trim().toUpperCase(), khz: spotFreqKhz(dialHz, audioHz, lsb, 2), info: modeName.trim() };
}

/** Lo spot via cluster telnet chiede un cluster collegato; via DXSummit no. */
export function spotBlocked(spotVia: 'dxsummit' | 'telnet' | undefined, connectedServers: number): boolean {
  return spotVia === 'telnet' && connectedServers === 0;
}
