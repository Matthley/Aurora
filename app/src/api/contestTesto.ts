/** Le chiamate dei contest dei modi testuali (`/api/v1/contest-testo`, vedi `core/crates/remote-api/src/contest_testo_api.rs`). */
import { ContestError } from './contest';

const BASE = '/api/v1/contest-testo';

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const r = await fetch(BASE + path, { credentials: 'include', ...init });
  if (!r.ok) {
    let code = 'error';
    let detail: string | undefined;
    try {
      const b = (await r.json()) as { error?: string; message?: string };
      code = b.error ?? code;
      detail = b.message;
    } catch {
      // corpo assente: resta il codice generico
    }
    throw new ContestError(r.status, code, detail);
  }
  return (await r.json()) as T;
}

const json = (method: string, body: unknown): RequestInit => ({ method, headers: { 'content-type': 'application/json' }, body: JSON.stringify(body) });

export interface BandMult {
  band: string;
  on: boolean;
  weight: number;
}

/** Un contest dei modi testuali, come nel file `.contest` (`docs/contest-file.md`). */
export interface TextContest {
  id: string;
  name: string;
  notes: string;
  /** `aaaammgghhmmss` UTC, vuoto = nessun limite. */
  start: string;
  end: string;
  work_time: number;
  nrs: string;
  auto_inc: boolean;
  valid_contacts: number;
  valid_cw: boolean;
  valid_phone: boolean;
  valid_digital: boolean;
  points_my_dxcc: number;
  points_my_cont: number;
  points_dx: number;
  multipliers: number;
  multipliers_option: number;
  band_mult: boolean;
  bands: BandMult[];
  macros: string;
  merge_macros: boolean;
  grab_number: boolean;
  grab_number_type: number;
  grab_time: boolean;
  grab_state: boolean;
  domain_file: string;
  grab_domain_to: number;
  suggest_cq_zone: boolean;
  suggest_itu_zone: boolean;
  suggest_domain: boolean;
  suggest_dxcc_pfx: boolean;
  dupe_no_band: boolean;
  dupe_no_mode: boolean;
  log_bar_mode: number;
}

export interface TextContestRef {
  id: string;
  name: string;
  notes: string;
  start: string;
  end: string;
}

export interface TextContestList {
  attivo: string | null;
  contest: TextContestRef[];
  macros: string[];
  domini: string[];
}

export interface ContestMacro {
  name: string;
  label: string;
  text: string;
  color: number | null;
}

export interface DomainEntry {
  key: string;
  value: string;
  section: string;
}

export interface Domain {
  title: string;
  country: string;
  entries: DomainEntry[];
}

/** Il contest attivo con le sue macro e il suo elenco dei domini. */
export interface ActiveTextContest {
  contest: TextContest;
  macros: ContestMacro[];
  domain: Domain | null;
  start_ms: number | null;
  end_ms: number | null;
}

export const getTextContests = () => call<TextContestList>('');
export const getActiveTextContest = () => call<ActiveTextContest | null>('/attivo');
export const getTextContest = (id: string) => call<TextContest>(`/voce/${encodeURIComponent(id)}`);
export const saveTextContest = (c: TextContest) => (c.id ? call<{ ok: boolean; id: string }>(`/voce/${encodeURIComponent(c.id)}`, json('PUT', c)) : call<{ ok: boolean; id: string }>('/voce', json('POST', c)));
export const deleteTextContest = (id: string) => call<{ ok: boolean }>(`/voce/${encodeURIComponent(id)}`, { method: 'DELETE' });
export const selectTextContest = (id: string | null) => call<{ ok: boolean; attivo: string | null }>('/seleziona', json('POST', { id }));

/** Un contest nuovo con i valori di base (digitali validi, incremento automatico, macro unite). */
export function newTextContest(): TextContest {
  return {
    id: '', name: '', notes: '', start: '', end: '', work_time: 0, nrs: '', auto_inc: true, valid_contacts: 0, valid_cw: false, valid_phone: false, valid_digital: true,
    points_my_dxcc: 0, points_my_cont: 0, points_dx: 0, multipliers: 0, multipliers_option: 0, band_mult: false,
    bands: ['160', '80', '40', '20', '15', '10'].map((band) => ({ band, on: false, weight: 0 })),
    macros: '', merge_macros: true, grab_number: true, grab_number_type: -1, grab_time: false, grab_state: false, domain_file: '', grab_domain_to: 0,
    suggest_cq_zone: false, suggest_itu_zone: false, suggest_domain: false, suggest_dxcc_pfx: false, dupe_no_band: false, dupe_no_mode: false, log_bar_mode: 0,
  };
}
