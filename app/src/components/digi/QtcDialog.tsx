import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useT } from '../../i18n';
import { ContestError } from '../../api/contest';
import { getQtc, saveQtc, type Qtc, type QtcBlock } from '../../api/contestTesto';
import { parseQtcSeries, qtcLineValid, qtcScripts, type QtcLine } from '../../lib/qtc';
import { toast } from '../../lib/toast';
import { TEXT_CONTEST_SCORE_KEY } from './ContestStatsDialog';
import '../../styles/digi-contest.css';

export type QtcDir = 'send' | 'receive';

interface Props {
  dir: QtcDir;
  /** La stazione del QSO (si puo' cambiare nella finestra). */
  call: string;
  mycall: string;
  band: string;
  mode: string;
  dialHz: number;
  /** Il testo della finestra di ricezione, per prendere la serie ricevuta. */
  rxText(): string;
  /** Esegue uno script di macro (trasmissione). */
  onRun(script: string): void;
  onClose(): void;
}

type SendRow = QtcLine & { qso_ms: number; on: boolean; ok: boolean };

const EMPTY: QtcLine = { ora: '', call: '', nr: '' };

/** Il motivo per cui la serie non si puo' scambiare, come chiave di testo. */
const blockKey = (b: QtcBlock) => (b === 'stesso_continente' ? 'qtc.blockContinent' : b === 'direzione_sbagliata' ? 'qtc.blockDirection' : 'qtc.blockDone');

/**
 * La finestra dei QTC del WAE. «Mandare»: i QSO del contest proposti per la stazione, la serie, i tasti per l'intestazione, tutte le righe
 * o una sola, e la spunta delle righe confermate che si salvano come mandate. «Ricevere»: QRV, le righe prese dalla ricezione (si
 * correggono), le richieste di ripetizione e il salvataggio con la conferma.
 */
export function QtcDialog({ dir: dir0, call: call0, mycall, band, mode, dialHz, rxText, onRun, onClose }: Props) {
  const t = useT();
  const qc = useQueryClient();
  const [dir, setDir] = useState<QtcDir>(dir0);
  const [call, setCall] = useState(call0.trim().toUpperCase());
  const q = useQuery({ queryKey: ['contest-testo-qtc', call], queryFn: () => getQtc(call), enabled: call.length >= 3, retry: false, refetchOnWindowFocus: false });
  const st = q.data?.stazione ?? null;
  const [send, setSend] = useState<SendRow[]>([]);
  const [serie, setSerie] = useState('');
  const [n, setN] = useState('');
  const [recv, setRecv] = useState<QtcLine[]>([]);
  const [busy, setBusy] = useState(false);

  // le proposte della stazione: tutte scelte, nessuna confermata
  useEffect(() => {
    setSend((st?.proposte ?? []).map((p) => ({ ...p, on: true, ok: false })));
  }, [st]);

  const chosen = useMemo(() => send.filter((r) => r.on), [send]);
  const block = dir === 'send' ? st?.invio : st?.ricezione;
  const residuo = st?.residuo ?? 10;
  const sendSerie = st?.serie ?? 1;

  const base = (r: QtcLine, extra: Partial<Qtc>): Qtc => ({
    inviato: dir === 'send', corrispondente: call, serie: 0, quanti: 0, time_ms: Date.now(), band, mode, freq_hz: Math.max(0, Math.round(dialHz)),
    ora: r.ora.trim(), call: r.call.trim().toUpperCase(), nr: r.nr.trim(), qso_ms: 0, ...extra,
  });

  const store = async (rows: Qtc[], after?: string) => {
    if (rows.length === 0) {
      toast(t('qtc.nothing'), 'info');
      return;
    }
    setBusy(true);
    try {
      const r = await saveQtc(rows);
      toast(t('qtc.saved', { n: rows.length, total: r.totale }), 'ok');
      if (after) onRun(after);
      void qc.invalidateQueries({ queryKey: ['contest-testo-qtc'] });
      void qc.invalidateQueries({ queryKey: [...TEXT_CONTEST_SCORE_KEY] });
      onClose();
    } catch (e) {
      toast(t('ctest.error', { why: e instanceof ContestError ? (e.detail ?? e.code) : String(e) }), 'error');
    } finally {
      setBusy(false);
    }
  };

  const saveSent = () => {
    const ok = chosen.filter((r) => r.ok);
    void store(ok.map((r) => base(r, { serie: sendSerie, quanti: chosen.length, qso_ms: r.qso_ms })));
  };

  const grab = () => {
    const s = parseQtcSeries(rxText().slice(-3000));
    if (s.serie != null) setSerie(String(s.serie));
    if (s.n != null) setN(String(s.n));
    if (s.rows.length === 0) toast(t('qtc.grabNone'), 'info');
    else setRecv(s.rows.slice(0, Math.min(10, residuo)));
  };

  const saveReceived = () => {
    const rows = recv.filter((r) => r.ora || r.call || r.nr);
    const sNum = Number(serie);
    const nNum = Number(n) || rows.length;
    if (!Number.isInteger(sNum) || sNum < 1) {
      toast(t('qtc.badSerie'), 'error');
      return;
    }
    const bad = rows.findIndex((r) => !qtcLineValid(r));
    if (bad >= 0) {
      toast(t('qtc.badLine', { n: bad + 1 }), 'error');
      return;
    }
    void store(rows.map((r) => base(r, { serie: sNum, quanti: nNum })), qtcScripts.confirm(call, mycall));
  };

  const setRecvRow = (i: number, k: keyof QtcLine, v: string) => setRecv((rs) => rs.map((r, j) => (j === i ? { ...r, [k]: v.toUpperCase() } : r)));
  const disabled = busy || !st || block != null;

  return (
    <div className="digi-modal" role="dialog" aria-modal="true" aria-label={t('qtc.title')} onKeyDown={(e) => (e.key === 'Escape' ? onClose() : undefined)} onClick={(e) => (e.target === e.currentTarget ? onClose() : undefined)}>
      <div className="digi-dialog ctest-dialog qtc-dialog">
        <header>
          <strong>{t('qtc.title')}</strong>
        </header>
        <div className="qtc-top">
          <div role="tablist" className="qtc-tabs">
            <button type="button" role="tab" aria-selected={dir === 'send'} className={`glass sstv-btn${dir === 'send' ? ' active' : ''}`} onClick={() => setDir('send')}>{t('qtc.send')}</button>
            <button type="button" role="tab" aria-selected={dir === 'receive'} className={`glass sstv-btn${dir === 'receive' ? ' active' : ''}`} onClick={() => setDir('receive')}>{t('qtc.receive')}</button>
          </div>
          <label>
            {t('qtc.station')} <input value={call} size={10} onChange={(e) => setCall(e.target.value.trim().toUpperCase())} />
          </label>
        </div>
        {call.length < 3 ? (
          <p>{t('qtc.noCall')}</p>
        ) : !q.data ? (
          <p>{q.isError ? t('ctest.error', { why: q.error instanceof ContestError ? (q.error.detail ?? q.error.code) : String(q.error) }) : t('ctest.loading')}</p>
        ) : q.data.regola == null ? (
          <p>{t('qtc.notWae')}</p>
        ) : (
          <>
            <p className="qtc-info">
              {t('qtc.info', { done: st?.scambiati ?? 0, left: residuo })}
              {st?.continente ? ` · ${st.mio_continente || '?'} → ${st.continente}` : ''}
            </p>
            {block ? <p className="qtc-block">{t(blockKey(block))}</p> : null}
            {dir === 'send' ? (
              send.length === 0 ? (
                <p>{t('qtc.noneToSend')}</p>
              ) : (
                <>
                  <p>{t('qtc.serie', { s: sendSerie, n: chosen.length })}</p>
                  <div className="ctest-table-wrap">
                    <table className="ctest-table">
                      <thead>
                        <tr>
                          <th>{t('qtc.c.use')}</th>
                          <th>#</th>
                          <th>{t('qtc.c.time')}</th>
                          <th>{t('qtc.c.call')}</th>
                          <th>{t('qtc.c.nr')}</th>
                          <th />
                          <th>QSL</th>
                        </tr>
                      </thead>
                      <tbody>
                        {send.map((r, i) => (
                          <tr key={`${r.call}-${r.qso_ms}`}>
                            <td><input type="checkbox" checked={r.on} aria-label={t('qtc.c.use')} onChange={(e) => setSend((rs) => rs.map((x, j) => (j === i ? { ...x, on: e.target.checked, ok: e.target.checked && x.ok } : x)))} /></td>
                            <td>{r.on ? chosen.indexOf(r) + 1 : ''}</td>
                            <td>{r.ora}</td>
                            <td>{r.call}</td>
                            <td>{r.nr}</td>
                            <td><button type="button" className="glass sstv-btn" disabled={disabled || !r.on} onClick={() => onRun(qtcScripts.line(r))}>{t('qtc.sendLine')}</button></td>
                            <td><input type="checkbox" checked={r.ok} disabled={!r.on} aria-label="QSL" onChange={(e) => setSend((rs) => rs.map((x, j) => (j === i ? { ...x, ok: e.target.checked } : x)))} /></td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <div className="qtc-actions">
                    <button type="button" className="glass sstv-btn" disabled={disabled || chosen.length === 0} onClick={() => onRun(qtcScripts.header(sendSerie, chosen.length))}>{t('qtc.header')}</button>
                    <button type="button" className="glass sstv-btn" disabled={disabled || chosen.length === 0} onClick={() => onRun(qtcScripts.all(sendSerie, chosen))}>{t('qtc.sendAll')}</button>
                    <button type="button" className="glass sstv-btn" disabled={disabled} onClick={() => onRun(qtcScripts.qsl())}>QSL?</button>
                    <button type="button" className="glass sstv-btn" disabled={disabled || chosen.length === 0} onClick={() => setSend((rs) => rs.map((x) => ({ ...x, ok: x.on })))}>{t('qtc.allOk')}</button>
                  </div>
                </>
              )
            ) : (
              <>
                <div className="qtc-actions">
                  <button type="button" className="glass sstv-btn" disabled={disabled} onClick={() => onRun(qtcScripts.qrv(mycall))}>QRV</button>
                  <button type="button" className="glass sstv-btn" disabled={disabled} onClick={grab}>{t('qtc.grab')}</button>
                  <button type="button" className="glass sstv-btn" disabled={disabled} onClick={() => onRun(qtcScripts.agnAll())}>AGN ALL</button>
                  <button type="button" className="glass sstv-btn" disabled={disabled} onClick={() => onRun(qtcScripts.agnHeader())}>AGN QTC</button>
                </div>
                <p className="qtc-serie-in">
                  <label>{t('qtc.serieIn')} <input value={serie} size={3} inputMode="numeric" onChange={(e) => setSerie(e.target.value.replace(/\D/g, ''))} /></label>
                  {' / '}
                  <label>{t('qtc.lines')} <input value={n} size={3} inputMode="numeric" onChange={(e) => setN(e.target.value.replace(/\D/g, ''))} /></label>
                </p>
                <div className="ctest-table-wrap">
                  <table className="ctest-table">
                    <thead>
                      <tr>
                        <th>#</th>
                        <th>{t('qtc.c.time')}</th>
                        <th>{t('qtc.c.call')}</th>
                        <th>{t('qtc.c.nr')}</th>
                        <th />
                      </tr>
                    </thead>
                    <tbody>
                      {recv.map((r, i) => (
                        <tr key={i} className={r.ora || r.call || r.nr ? (qtcLineValid(r) ? '' : 'qtc-bad') : ''}>
                          <td>{i + 1}</td>
                          <td><input value={r.ora} size={4} aria-label={t('qtc.c.time')} onChange={(e) => setRecvRow(i, 'ora', e.target.value)} /></td>
                          <td><input value={r.call} size={10} aria-label={t('qtc.c.call')} onChange={(e) => setRecvRow(i, 'call', e.target.value)} /></td>
                          <td><input value={r.nr} size={4} aria-label={t('qtc.c.nr')} onChange={(e) => setRecvRow(i, 'nr', e.target.value)} /></td>
                          <td>
                            <button type="button" className="glass sstv-btn" disabled={disabled} onClick={() => onRun(qtcScripts.agn(i + 1))}>AGN</button>
                            <button type="button" className="glass sstv-btn" onClick={() => setRecv((rs) => rs.filter((_, j) => j !== i))} aria-label={t('qtc.delLine')}>×</button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
                <button type="button" className="glass sstv-btn" disabled={recv.length >= Math.min(10, residuo)} onClick={() => setRecv((rs) => [...rs, { ...EMPTY }])}>{t('qtc.addLine')}</button>
              </>
            )}
          </>
        )}
        <footer>
          {dir === 'send' ? (
            <button type="button" className="glass sstv-btn" disabled={disabled || !chosen.some((r) => r.ok)} onClick={saveSent}>{t('qtc.saveSent')}</button>
          ) : (
            <button type="button" className="glass sstv-btn" disabled={disabled || recv.length === 0} onClick={saveReceived}>{t('qtc.saveReceived')}</button>
          )}
          <span className="rtty-spacer" />
          <button type="button" className="glass sstv-btn" onClick={onClose}>{t('ctest.close')}</button>
        </footer>
      </div>
    </div>
  );
}
