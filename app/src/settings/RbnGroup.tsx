import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { getRbn, putRbn, type RbnConfig } from '../api/client';
import { useT } from '../i18n';
import { Check } from './common';
import { Cells, Field, Group, Message } from './layout';

const BANDS = ['160m', '80m', '60m', '40m', '30m', '20m', '17m', '15m', '12m', '10m', '6m'];
const MODES = ['CW', 'RTTY', 'FT8', 'FT4'];
const CONTINENTS = ['EU', 'NA', 'SA', 'AS', 'AF', 'OC'];

/** Una fila di caselle: nessuna scelta = tutte. */
function Picks({ all, chosen, onChange, label }: { all: string[]; chosen: string[]; onChange(next: string[]): void; label: string }) {
  return (
    <div className="cmd-line" role="group" aria-label={label}>
      {all.map((v) => (
        <label key={v} className="set-check">
          <input type="checkbox" checked={chosen.includes(v)} onChange={(e) => onChange(e.target.checked ? [...chosen, v] : chosen.filter((x) => x !== v))} />
          <span>{v}</span>
        </label>
      ))}
    </div>
  );
}

/**
 * Reverse Beacon Network: ricezione degli spot dei ricevitori automatici (porte 7000 e 7001, si entra con il nominativo di stazione) e il
 * server spot locale che offre ai programmi di log le stazioni in CQ verificate da Aurora BIGScan. Tutto spento di fabbrica; ogni modifica
 * si salva subito.
 */
export function RbnGroup({ callsign }: { callsign?: string }) {
  const t = useT();
  const qc = useQueryClient();
  const state = useQuery({ queryKey: ['rbn'], queryFn: getRbn, retry: false, refetchInterval: 5_000 });
  const [draft, setDraft] = useState<RbnConfig | null>(null);
  const [port, setPort] = useState('');
  useEffect(() => {
    if (state.data && draft === null) {
      setDraft(state.data.config);
      setPort(String(state.data.config.server_port));
    }
  }, [state.data, draft]);
  const save = useMutation({ mutationFn: putRbn, onSuccess: () => qc.invalidateQueries({ queryKey: ['rbn'] }) });
  if (state.isError || !state.data || !draft) return null;
  const set = (change: Partial<RbnConfig>) => {
    const next = { ...draft, ...change };
    setDraft(next);
    save.mutate(next);
  };
  const portOk = /^\d+$/.test(port) && Number(port) >= 1024 && Number(port) <= 65535;
  const stateText = (s: 'connecting' | 'connected' | 'error') => t(s === 'connected' ? 'rbn.st.connected' : s === 'connecting' ? 'rbn.st.connecting' : 'rbn.st.error');
  const server = state.data.server;

  return (
    <>
      <Group title={t('rbn.title')} span={2}>
        <Message kind="muted">{t('rbn.help')}</Message>
        <Cells min={13}>
          <Check label={t('rbn.enabled')} hint={t('rbn.enabledTip')} checked={draft.enabled} onChange={(v) => set({ enabled: v })} />
          <Check label={t('rbn.cwRtty')} checked={draft.cw_rtty} onChange={(v) => set({ cw_rtty: v })} />
          <Check label={t('rbn.digital')} checked={draft.digital} onChange={(v) => set({ digital: v })} />
        </Cells>
        {draft.enabled && !callsign ? <Message kind="alert">{t('rbn.callMissing')}</Message> : null}
        {state.data.feeds.map((f) => (
          <Message key={f.port} kind={f.state === 'error' ? 'alert' : 'muted'}>
            {t('rbn.feed', { port: f.port, state: stateText(f.state), n: f.spots })}
            {f.detail ? ` · ${f.detail}` : ''}
          </Message>
        ))}
        <Field label={t('rbn.bands')} stack wide>
          <Picks all={BANDS} chosen={draft.bands} label={t('rbn.bands')} onChange={(bands) => set({ bands })} />
        </Field>
        <Field label={t('rbn.modes')} stack wide>
          <Picks all={MODES} chosen={draft.modes} label={t('rbn.modes')} onChange={(modes) => set({ modes })} />
        </Field>
        <Field label={t('rbn.continents')} stack wide>
          <Picks all={CONTINENTS} chosen={draft.continents} label={t('rbn.continents')} onChange={(continents) => set({ continents })} />
        </Field>
      </Group>

      <Group title={t('rbn.server.title')} span={2}>
        <Message kind="muted">{t('rbn.server.help')}</Message>
        <Cells min={13}>
          <Check label={t('rbn.server.enabled')} checked={draft.server_enabled} onChange={(v) => set({ server_enabled: v })} />
          <Field label={t('rbn.server.port')}>
            <input
              type="number"
              className="set-number"
              min={1024}
              max={65535}
              value={port}
              onChange={(e) => setPort(e.target.value)}
              onBlur={() => {
                if (portOk && Number(port) !== draft.server_port) set({ server_port: Number(port) });
              }}
            />
          </Field>
          <Check label={t('rbn.server.lan')} hint={t('rbn.server.lanTip')} checked={draft.server_lan} onChange={(v) => set({ server_lan: v })} />
        </Cells>
        {!portOk ? <Message kind="alert">{t('rbn.server.badPort')}</Message> : null}
        {server === 'listening' ? <Message>{t('rbn.server.listening', { port: draft.server_port, n: state.data.clients })}</Message> : server ? <Message kind="alert">{server}</Message> : null}
      </Group>
    </>
  );
}
