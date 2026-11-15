import { useT } from '../i18n';

/**
 * Sul client remoto (telefono, portatile in rete) aprire la pagina non accende il motore: l'ascolto parte solo premendo «Ascolta».
 * Sul PC principale questo riquadro non compare e l'ascolto parte entrando, come sempre.
 */
export function ListenGate({ onListen }: { onListen: () => void }) {
  const t = useT();
  return (
    <div className="glass listen-gate" role="status" style={{ display: 'flex', alignItems: 'center', gap: '0.75rem', padding: '0.6rem 0.9rem', margin: '0.5rem 0' }}>
      <span style={{ flex: 1 }}>{t('remote.listen.hint')}</span>
      <button type="button" className="glass" onClick={onListen}>
        {t('remote.listen')}
      </button>
    </div>
  );
}
