import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { connectCat, disconnectCat, getCat, type CatSnapshot } from '../api/client';
import { useT } from '../i18n';

/**
 * Collega e scollega il CAT dalla barra dei comandi, senza aprire le Impostazioni: utile quando un altro programma deve usare la stessa
 * radio. Acceso (verde) se la radio e' collegata; un clic scollega, un altro ricollega. Senza un sistema CAT scelto non si mostra.
 */
export function CatSwitch() {
  const t = useT();
  const client = useQueryClient();
  const cat = useQuery({ queryKey: ['cat'], queryFn: getCat, retry: false, refetchInterval: 3000 });
  const [busy, setBusy] = useState(false);
  const snap = cat.data;
  if (!snap || snap.config.backend === 'none') return null;
  const state = snap.status.state;
  const on = state === 'connected';
  const trying = busy || state === 'connecting' || state === 'reconnecting';
  const toggle = async () => {
    setBusy(true);
    try {
      const next: CatSnapshot = on || trying ? await disconnectCat() : await connectCat();
      client.setQueryData(['cat'], next);
    } catch {
      // lo stato vero arriva con la rilettura qui sotto
    } finally {
      setBusy(false);
      void client.invalidateQueries({ queryKey: ['cat'] });
    }
  };
  return (
    <button
      type="button"
      className={`rx-seg-item${on ? ' rx-seg-item--on' : ''}`}
      aria-pressed={on}
      disabled={busy}
      title={t(on ? 'ctl.cat.tipOn' : 'ctl.cat.tipOff')}
      onClick={() => void toggle()}
    >
      {t('ctl.cat')}
    </button>
  );
}
