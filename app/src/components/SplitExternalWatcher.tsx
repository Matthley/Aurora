import { useQuery } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { getCat } from '../api/client';
import { useT } from '../i18n';
import { toast } from '../lib/toast';

/**
 * Avvisa quando la radio va in split senza un comando di Aurora (la radio a mano o un programma collegato alla condivisione di
 * `rigctld`): il core segna l'ora in `split_external_ms`. Sempre montato, anche con il display della radio nascosto. Il valore gia'
 * presente all'apertura della pagina non si mostra: solo quelli nuovi.
 */
export function SplitExternalWatcher() {
  const t = useT();
  const cat = useQuery({ queryKey: ['cat'], queryFn: getCat, retry: false, refetchInterval: 5000 });
  const at = cat.data?.status.rig?.split_external_ms ?? 0;
  const seen = useRef<number | null>(null);
  useEffect(() => {
    if (!cat.data) return;
    if (seen.current === null) {
      seen.current = at;
      return;
    }
    if (at > seen.current) {
      seen.current = at;
      toast(t('rd.splitExternal'), 'error');
    }
  }, [at, cat.data, t]);
  return null;
}
