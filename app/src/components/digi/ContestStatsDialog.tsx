import { useQuery } from '@tanstack/react-query';
import { useT } from '../../i18n';
import { ContestError } from '../../api/contest';
import { getTextContestCabrillo, getTextContestScore, type TextContestScore } from '../../api/contestTesto';
import { saveText } from '../../lib/foxLog';
import { toast } from '../../lib/toast';
import '../../styles/digi-contest.css';

/** La chiave del punteggio del contest attivo dei modi testuali (da rileggere dopo ogni QSO salvato). */
export const TEXT_CONTEST_SCORE_KEY = ['contest-testo-punteggio'] as const;

/** Il punteggio del contest attivo, letto dal core ogni 30 secondi e dopo ogni QSO salvato. */
export function useTextContestScore(active: boolean) {
  return useQuery({ queryKey: [...TEXT_CONTEST_SCORE_KEY], queryFn: getTextContestScore, enabled: active, retry: false, refetchInterval: 30_000, refetchOnWindowFocus: false });
}

/** Salva il Cabrillo del contest attivo (il core lo scrive dai QSO del log). */
export function useExportTextCabrillo() {
  const t = useT();
  return async () => {
    try {
      const r = await getTextContestCabrillo();
      if (r.qso === 0) {
        toast(t('ctest.cabrilloEmpty'), 'info');
        return;
      }
      saveText(r.nome, r.testo);
      toast(t('ctest.cabrilloSaved', { name: r.nome, n: r.qso }), 'ok');
    } catch (e) {
      toast(t('ctest.error', { why: e instanceof ContestError ? (e.detail ?? e.code) : String(e) }), 'error');
    }
  };
}

/** La riga del punteggio: punti × moltiplicatori = punteggio (o i soli punti se il contest non ha moltiplicatori). */
export function scoreLine(t: ReturnType<typeof useT>, s: TextContestScore): string {
  return s.tipi.length ? t('ctest.scoreLine', { points: s.punti, mult: s.molt, score: s.punteggio }) : t('ctest.scoreNoMult', { score: s.punteggio });
}

/** La finestrella delle statistiche del contest attivo: per banda QSO, doppi, punti, moltiplicatori e categorie, poi il totale. */
export function ContestStatsDialog({ name, onClose }: { name: string; onClose(): void }) {
  const t = useT();
  const exportCabrillo = useExportTextCabrillo();
  const q = useTextContestScore(true);
  const s = q.data;
  const sum = (k: 'cw' | 'phone' | 'digital') => s?.bande.reduce((a, b) => a + b[k], 0) ?? 0;
  return (
    <div className="digi-modal" role="dialog" aria-modal="true" aria-label={t('ctest.statsTitle', { name })} onKeyDown={(e) => (e.key === 'Escape' ? onClose() : undefined)} onClick={(e) => (e.target === e.currentTarget ? onClose() : undefined)}>
      <div className="digi-dialog ctest-dialog">
        <header>
          <strong>{t('ctest.statsTitle', { name })}</strong>
        </header>
        {!s ? (
          <p>{q.isError ? t('ctest.error', { why: q.error instanceof ContestError ? (q.error.detail ?? q.error.code) : String(q.error) }) : t('ctest.loading')}</p>
        ) : s.qso + s.doppi === 0 ? (
          <p>{t('ctest.noQsos')}</p>
        ) : (
          <>
            <div className="ctest-table-wrap">
              <table className="ctest-table ctest-stats">
                <thead>
                  <tr>
                    <th>{t('ctest.c.band')}</th>
                    <th>{t('ctest.c.qso')}</th>
                    <th>{t('ctest.c.dupes')}</th>
                    <th>{t('ctest.c.points')}</th>
                    <th>{t('ctest.c.mult')}</th>
                    <th>CW</th>
                    <th>{t('ctest.c.phone')}</th>
                    <th>{t('ctest.c.digital')}</th>
                  </tr>
                </thead>
                <tbody>
                  {s.bande.map((b) => (
                    <tr key={b.band}>
                      <td>{b.band || '—'}</td>
                      <td>{b.qso}</td>
                      <td>{b.doppi}</td>
                      <td>{b.punti}</td>
                      <td>{b.molt}</td>
                      <td>{b.cw}</td>
                      <td>{b.phone}</td>
                      <td>{b.digital}</td>
                    </tr>
                  ))}
                  <tr className="ctest-total">
                    <td>{t('ctest.c.total')}</td>
                    <td>{s.qso}</td>
                    <td>{s.doppi}</td>
                    <td>{s.punti}</td>
                    <td>{s.molt}</td>
                    <td>{sum('cw')}</td>
                    <td>{sum('phone')}</td>
                    <td>{sum('digital')}</td>
                  </tr>
                </tbody>
              </table>
            </div>
            <p className="ctest-score">{scoreLine(t, s)}</p>
          </>
        )}
        <footer>
          <button type="button" className="glass sstv-btn" disabled={!s || s.qso + s.doppi === 0} onClick={() => void exportCabrillo()}>
            {t('ctest.cabrillo')}
          </button>
          <span className="rtty-spacer" />
          <button type="button" className="glass sstv-btn" onClick={onClose}>
            {t('ctest.close')}
          </button>
        </footer>
      </div>
    </div>
  );
}
