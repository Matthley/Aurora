import { useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { ApiError, postSplitOff, type CatSnapshot, type SplitDx } from '../../api/client';
import { useT } from '../../i18n';
import { toast } from '../../lib/toast';

/**
 * Tasto SPLIT nel blocco a destra del display, sotto PWR, nello stile dei tasti del display: acceso con la radio in split (VFO B in
 * trasmissione), un clic la riporta in simplex qualunque cosa l'avesse accesa (uno spot «UP», lo split di FT8, la radio o un altro
 * programma). Spento e grigio con la radio in simplex. Richiesta del proprietario: deve esserci sempre un modo veloce per spegnerlo.
 */
export function SplitButton({ split }: { split: boolean | null | undefined }) {
  const t = useT();
  const client = useQueryClient();
  const [busy, setBusy] = useState(false);
  const on = split === true;
  // radio spenta o che non risponde (split sconosciuto): il pulsante non serve e non compare
  if (split === null || split === undefined) return null;
  const off = async () => {
    setBusy(true);
    // il display mostra subito lo split spento (prima aspettava la risposta e poi la rilettura del CAT, fino a 3 s); se la radio rifiuta torna com'era
    await client.cancelQueries({ queryKey: ['cat'] });
    const before = client.getQueryData<CatSnapshot>(['cat']);
    const beforeDx = client.getQueryData<SplitDx>(['split-dx']);
    client.setQueryData<CatSnapshot>(['cat'], (c) => (c?.status.rig ? { ...c, status: { ...c.status, rig: { ...c.status.rig, split: false } } } : c));
    if (beforeDx) client.setQueryData<SplitDx>(['split-dx'], { active: false });
    try {
      await postSplitOff();
    } catch (e) {
      if (before) client.setQueryData(['cat'], before);
      if (beforeDx) client.setQueryData(['split-dx'], beforeDx);
      toast(e instanceof ApiError ? (e.message ?? e.code) : String(e), 'error');
    } finally {
      setBusy(false);
      // conferma dalla radio
      void client.invalidateQueries({ queryKey: ['cat'] });
      void client.invalidateQueries({ queryKey: ['split-dx'] });
    }
  };
  return (
    <button
      type="button"
      className={`rd-split-btn${on ? ' rd-split-btn--on' : ''}`}
      title={t('rd.splitOff.tip')}
      aria-label={t('rd.splitOff')}
      aria-pressed={on}
      disabled={busy || !on}
      onClick={() => void off()}
    >
      SPLIT
    </button>
  );
}
