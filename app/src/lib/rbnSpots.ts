import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getRbn, getRbnSpots, type RbnSpot } from '../api/client';
import { bandOf } from './format';
import type { WaterfallLabel } from './digiBand';

/**
 * Le stazioni sentite dalla rete RBN nel tratto della cascata (radio + `minHz`..`maxHz`), della banda e del modo della pagina, che Aurora
 * non ha decodificato (`known`: i nominativi gia' nell'elenco di Aurora BIGScan). Niente richieste se spento, senza radio o con la
 * ricezione RBN spenta nelle impostazioni.
 */
export function useRbnSpots(on: boolean, dialHz: number, mode: string, minHz: number, maxHz: number, known: ReadonlySet<string>): RbnSpot[] {
  const enabled = useQuery({ queryKey: ['rbn'], queryFn: getRbn, retry: false, enabled: on, refetchInterval: 60_000 }).data?.config.enabled === true;
  const band = dialHz > 0 ? bandOf(dialHz) : null;
  // la finestra si arrotonda al kHz: piccoli spostamenti della radio non rifanno la richiesta
  const lo = Math.floor((dialHz + minHz) / 1000);
  const hi = Math.ceil((dialHz + maxHz) / 1000);
  const spots = useQuery({
    queryKey: ['rbn-spots', band, mode, lo, hi],
    queryFn: () => getRbnSpots(band ?? '', mode, lo, hi),
    enabled: on && enabled && band !== null,
    retry: false,
    refetchInterval: 15_000,
    placeholderData: (prev) => prev,
  }).data;
  return useMemo(() => (on && enabled && spots ? spots.filter((s) => !known.has(s.call.toUpperCase())) : []), [on, enabled, spots, known]);
}

/** Le etichette RBN sulla cascata (tono audio = frequenza dello spot meno la radio, in USB). */
export function rbnLabels(spots: RbnSpot[], dialHz: number): WaterfallLabel[] {
  if (dialHz <= 0) return [];
  return spots.map((s) => ({ hz: s.freq_khz * 1000 - dialHz, text: s.call, cq: s.kind === 'cq', sure: true, rbn: true, history: `RBN · ${s.skimmers} · ${s.best_snr_db ?? '?'} dB (${s.best_skimmer})` }));
}
