import { browserStorage } from './theme';

/**
 * Misure dei pannelli dei modi testuali (cascata, RX/TX, Radar, macro, log) ricordate per modo: la chiave diventa `<chiave>.<modo>`
 * (per esempio `aurora.digi.wfh2.cw`). Se il modo non ha ancora una misura propria vale quella vecchia, comune a tutti i modi.
 */
const keyOf = (key: string, mode: string) => `${key}.${mode}`;

export function readSize(key: string, mode: string): number {
  try {
    const s = browserStorage();
    return Number(s?.getItem(keyOf(key, mode)) ?? s?.getItem(key) ?? '');
  } catch {
    return Number.NaN;
  }
}

export function writeSize(key: string, mode: string, value: number): void {
  try {
    browserStorage()?.setItem(keyOf(key, mode), String(value));
  } catch {
    // la misura vale comunque finche' la pagina e' aperta
  }
}

/** Torna alla misura di partenza per quel modo (anche quella comune vecchia non vale piu' per lui). */
export function clearSize(key: string, mode: string): void {
  try {
    browserStorage()?.setItem(keyOf(key, mode), '');
  } catch {
    // niente da togliere
  }
}
