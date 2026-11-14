import type { RbnSpot } from '../../api/client';
import { useT } from '../../i18n';

/** Le stazioni sentite dalla rete RBN e non decodificate da Aurora, sotto Aurora BIGScan: badge «RBN», ricevitori, S/N migliore. Clic: ricezione li'. */
export function RbnList({ spots, dialHz, onTune }: { spots: RbnSpot[]; dialHz: number; onTune(hz: number): void }) {
  const t = useT();
  if (spots.length === 0 || dialHz <= 0) return null;
  return (
    <div className="rbn-list" title={t('rbn.listTip')}>
      <h4>{t('rbn.listTitle')}</h4>
      <ul>
        {spots.map((s) => (
          <li key={`${s.call}-${s.mode}`}>
            <button
              type="button"
              title={t('rbn.spotTip', { call: s.call, freq: s.freq_khz.toFixed(1), n: s.receivers, snr: s.best_snr_db ?? '?', receiver: s.best_receiver, time: s.time_utc })}
              onClick={() => onTune(s.freq_khz * 1000 - dialHz)}
            >
              <span className="rbn-badge">{t('rbn.badge')}</span>
              <span className="rbn-call">{s.call}</span>
              <span className="data">{s.freq_khz.toFixed(1)}</span>
              <span className="data muted">×{s.receivers}</span>
              <span className="data muted">{s.best_snr_db ?? '?'} dB</span>
              {s.speed ? <span className="data muted">{s.speed}</span> : null}
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
