import { useState, type ReactNode } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useT } from '../../i18n';
import { ContestError } from '../../api/contest';
import { deleteTextContest, getTextContest, getTextContests, newTextContest, saveTextContest, selectTextContest, type TextContest } from '../../api/contestTesto';
import { askConfirm } from '../ConfirmHost';
import { toast } from '../../lib/toast';
import '../../styles/digi-contest.css';

const why = (e: unknown) => (e instanceof ContestError ? (e.detail ?? e.code) : String(e));

/** Le chiavi delle richieste dei contest dei modi testuali (elenco e contest attivo), da rileggere dopo ogni modifica. */
export const TEXT_CONTEST_KEYS = [['contest-testo'], ['contest-testo-attivo']] as const;

function useRefresh() {
  const client = useQueryClient();
  return () => {
    for (const k of TEXT_CONTEST_KEYS) void client.invalidateQueries({ queryKey: [...k] });
  };
}

/** Sceglie il contest attivo (`null` = «Nessun contest») e lo dice. */
export function useSelectTextContest() {
  const t = useT();
  const refresh = useRefresh();
  return async (id: string | null, name?: string) => {
    try {
      await selectTextContest(id);
      toast(id ? t('ctest.selected', { name: name ?? id }) : t('ctest.deselected'), 'info');
      refresh();
      return true;
    } catch (e) {
      toast(t('ctest.error', { why: why(e) }), 'error');
      return false;
    }
  };
}

/**
 * La finestra «Elenco dei contest»: colonne Attivo (freccia su quello attivo), Nome, Note; prima riga «Nessun contest»; pulsanti
 * Modifica…, Elimina, Aggiungi…, Seleziona, Chiudi. Doppio clic su una riga = Seleziona.
 */
export function ContestListDialog({ onClose, onSelected }: { onClose(): void; onSelected?(id: string | null): void }) {
  const t = useT();
  const refresh = useRefresh();
  const select = useSelectTextContest();
  const list = useQuery({ queryKey: ['contest-testo'], queryFn: getTextContests, retry: false });
  const [picked, setPicked] = useState<string | null | undefined>(undefined);
  const [editing, setEditing] = useState<TextContest | null>(null);
  const active = list.data?.attivo ?? null;
  const current = picked === undefined ? active : picked;
  const rows = [{ id: null as string | null, name: t('ctest.none'), notes: '' }, ...(list.data?.contest ?? [])];

  const choose = async (id: string | null) => {
    const name = rows.find((r) => r.id === id)?.name;
    if (await select(id, name ?? undefined)) {
      onSelected?.(id);
      onClose();
    }
  };
  const edit = async (id: string | null) => {
    if (!id) return;
    try {
      setEditing(await getTextContest(id));
    } catch (e) {
      toast(t('ctest.error', { why: why(e) }), 'error');
    }
  };
  const remove = async (id: string | null) => {
    if (!id) return;
    const name = rows.find((r) => r.id === id)?.name ?? id;
    if (!(await askConfirm(t('ctest.deleteConfirm', { name })))) return;
    try {
      await deleteTextContest(id);
      toast(t('ctest.deleted'), 'info');
      setPicked(undefined);
      refresh();
    } catch (e) {
      toast(t('ctest.error', { why: why(e) }), 'error');
    }
  };

  return (
    <div className="digi-modal" role="dialog" aria-modal="true" aria-label={t('ctest.listTitle')} onKeyDown={(e) => (e.key === 'Escape' && !editing ? onClose() : undefined)} onClick={(e) => (e.target === e.currentTarget ? onClose() : undefined)}>
      <div className="digi-dialog ctest-dialog">
        <header>
          <strong>{t('ctest.listTitle')}</strong>
        </header>
        <div className="ctest-table-wrap">
          <table className="ctest-table">
            <thead>
              <tr>
                <th className="ctest-col-active">{t('ctest.colActive')}</th>
                <th>{t('ctest.colName')}</th>
                <th>{t('ctest.colNotes')}</th>
              </tr>
            </thead>
            <tbody>
              {list.isLoading ? (
                <tr>
                  <td colSpan={3}>{t('ctest.loading')}</td>
                </tr>
              ) : (
                rows.map((r) => (
                  <tr key={r.id ?? ''} className={r.id === current ? 'is-picked' : undefined} aria-selected={r.id === current} onClick={() => setPicked(r.id)} onDoubleClick={() => void choose(r.id)}>
                    <td className="ctest-col-active">{r.id === active ? '►' : ''}</td>
                    <td>{r.name}</td>
                    <td>{r.notes}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
        <footer>
          <button type="button" className="glass sstv-btn" disabled={!current} onClick={() => void edit(current)}>
            {t('ctest.edit')}
          </button>
          <button type="button" className="glass sstv-btn" disabled={!current} onClick={() => void remove(current)}>
            {t('ctest.delete')}
          </button>
          <button type="button" className="glass sstv-btn" onClick={() => setEditing(newTextContest())}>
            {t('ctest.add')}
          </button>
          <span className="rtty-spacer" />
          <button type="button" className="glass sstv-btn" onClick={() => void choose(current)}>
            {t('ctest.select')}
          </button>
          <button type="button" className="glass sstv-btn" onClick={onClose}>
            {t('ctest.close')}
          </button>
        </footer>
      </div>
      {editing ? (
        <ContestEditDialog
          contest={editing}
          onClose={() => setEditing(null)}
          onSaved={(id) => {
            setPicked(id);
            setEditing(null);
          }}
        />
      ) : null}
    </div>
  );
}

/** Un gruppo della finestra di modifica. */
function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="ctest-group">
      <legend>{title}</legend>
      {children}
    </fieldset>
  );
}

/** `aaaammgghhmmss` <-> valore di `<input type="datetime-local">` (UTC). */
export function toLocalInput(d: string): string {
  const m = /^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})$/.exec(d.trim());
  return m ? `${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}` : '';
}

export function fromLocalInput(v: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})(?::(\d{2}))?/.exec(v.trim());
  return m ? `${m[1]}${m[2]}${m[3]}${m[4]}${m[5]}${m[6] ?? '00'}` : '';
}

/** La finestra di modifica di un contest (nuovo se senza id): tutti i campi del file, in gruppi, con le spiegazioni. */
export function ContestEditDialog({ contest, onClose, onSaved }: { contest: TextContest; onClose(): void; onSaved(id: string): void }) {
  const t = useT();
  const list = useQuery({ queryKey: ['contest-testo'], queryFn: getTextContests, retry: false }).data;
  const macroFiles = list?.macros ?? [];
  const domainFiles = list?.domini ?? [];
  const refresh = useRefresh();
  const [c, setC] = useState<TextContest>(contest);
  const [busy, setBusy] = useState(false);
  const set = (p: Partial<TextContest>) => setC((x) => ({ ...x, ...p }));
  const num = (v: string) => (Number.isFinite(Number(v)) ? Math.trunc(Number(v)) : 0);
  const check = (k: keyof TextContest, label: string, tip?: string) => (
    <label className="ctest-check" title={tip}>
      <input type="checkbox" checked={c[k] === true} onChange={(e) => set({ [k]: e.target.checked } as Partial<TextContest>)} />
      <span>{label}</span>
    </label>
  );
  const int = (k: keyof TextContest, label: string, tip?: string) => (
    <label title={tip}>
      <span>{label}</span>
      <input type="number" value={Number(c[k])} onChange={(e) => set({ [k]: num(e.target.value) } as Partial<TextContest>)} />
    </label>
  );
  const save = async () => {
    setBusy(true);
    try {
      const r = await saveTextContest(c);
      toast(t('ctest.saved'), 'info');
      refresh();
      onSaved(r.id);
    } catch (e) {
      toast(t('ctest.error', { why: why(e) }), 'error');
    } finally {
      setBusy(false);
    }
  };
  const files = (list: readonly string[], current: string) => (current && !list.includes(current) ? [current, ...list] : list);
  const stored = t('ctest.tip.stored');
  return (
    <div className="digi-modal" role="dialog" aria-modal="true" aria-label={c.id ? t('ctest.editTitle', { name: c.name }) : t('ctest.newTitle')} onKeyDown={(e) => (e.key === 'Escape' ? (e.stopPropagation(), onClose()) : undefined)} onClick={(e) => (e.target === e.currentTarget ? onClose() : undefined)}>
      <div className="digi-dialog ctest-dialog ctest-edit">
        <header>
          <strong>{c.id ? t('ctest.editTitle', { name: contest.name }) : t('ctest.newTitle')}</strong>
        </header>
        <div className="ctest-groups">
          <Group title={t('ctest.g.general')}>
            <label>
              <span>{t('ctest.f.name')}</span>
              <input type="text" value={c.name} onChange={(e) => set({ name: e.target.value })} autoFocus />
            </label>
            <label>
              <span>{t('ctest.f.notes')}</span>
              <input type="text" value={c.notes} onChange={(e) => set({ notes: e.target.value })} />
            </label>
            <label title={t('ctest.tip.dates')}>
              <span>{t('ctest.f.start')}</span>
              <input type="datetime-local" step={1} value={toLocalInput(c.start)} onChange={(e) => set({ start: fromLocalInput(e.target.value) })} />
            </label>
            <label title={t('ctest.tip.dates')}>
              <span>{t('ctest.f.end')}</span>
              <input type="datetime-local" step={1} value={toLocalInput(c.end)} onChange={(e) => set({ end: fromLocalInput(e.target.value) })} />
            </label>
            {int('work_time', t('ctest.f.workTime'), t('ctest.tip.workTime'))}
          </Group>
          <Group title={t('ctest.g.serial')}>
            <label title={t('ctest.tip.nrs')}>
              <span>{t('ctest.f.nrs')}</span>
              <input type="text" value={c.nrs} onChange={(e) => set({ nrs: e.target.value })} />
            </label>
            {check('auto_inc', t('ctest.f.autoInc'), t('ctest.tip.autoInc'))}
          </Group>
          <Group title={t('ctest.g.validity')}>
            {check('valid_cw', t('ctest.f.validCw'), t('ctest.tip.valid'))}
            {check('valid_phone', t('ctest.f.validPhone'), t('ctest.tip.valid'))}
            {check('valid_digital', t('ctest.f.validDigital'), t('ctest.tip.valid'))}
            {int('valid_contacts', t('ctest.f.validContacts'), stored)}
          </Group>
          <Group title={t('ctest.g.points')}>
            {int('points_my_dxcc', t('ctest.f.pointsMyDxcc'), stored)}
            {int('points_my_cont', t('ctest.f.pointsMyCont'), stored)}
            {int('points_dx', t('ctest.f.pointsDx'), stored)}
          </Group>
          <Group title={t('ctest.g.mult')}>
            {int('multipliers', t('ctest.f.multipliers'), stored)}
            {int('multipliers_option', t('ctest.f.multOption'), stored)}
            {check('band_mult', t('ctest.f.bandMult'), stored)}
            <div className="ctest-bands">
              {c.bands.map((b, i) => (
                <div key={b.band} className="ctest-band" title={stored}>
                  <label className="ctest-check">
                    <input type="checkbox" checked={b.on} onChange={(e) => set({ bands: c.bands.map((x, j) => (j === i ? { ...x, on: e.target.checked } : x)) })} />
                    <span>{t('ctest.f.band', { band: b.band })}</span>
                  </label>
                  <input type="number" aria-label={`${t('ctest.f.weight')} ${b.band}`} value={b.weight} onChange={(e) => set({ bands: c.bands.map((x, j) => (j === i ? { ...x, weight: num(e.target.value) } : x)) })} />
                </div>
              ))}
            </div>
          </Group>
          <Group title={t('ctest.g.macros')}>
            <label title={t('ctest.tip.macros')}>
              <span>{t('ctest.f.macros')}</span>
              <select value={c.macros} onChange={(e) => set({ macros: e.target.value })}>
                <option value="">{t('ctest.f.macrosNone')}</option>
                {files(macroFiles, c.macros).map((f) => (
                  <option key={f} value={f}>
                    {f.replace(/^macros\//, '')}
                  </option>
                ))}
              </select>
            </label>
            {check('merge_macros', t('ctest.f.merge'), t('ctest.tip.merge'))}
          </Group>
          <Group title={t('ctest.g.grab')}>
            {check('grab_number', t('ctest.f.grabNumber'), t('ctest.tip.grabNumber'))}
            {int('grab_number_type', t('ctest.f.grabNumberType'), stored)}
            {check('grab_time', t('ctest.f.grabTime'), stored)}
            {check('grab_state', t('ctest.f.grabState'), t('ctest.tip.grabState'))}
            <label title={t('ctest.tip.domain')}>
              <span>{t('ctest.f.domain')}</span>
              <select value={c.domain_file} onChange={(e) => set({ domain_file: e.target.value })}>
                <option value="">{t('ctest.f.macrosNone')}</option>
                {files(domainFiles, c.domain_file).map((f) => (
                  <option key={f} value={f}>
                    {f.replace(/^dom\//, '')}
                  </option>
                ))}
              </select>
            </label>
            {int('grab_domain_to', t('ctest.f.grabDomainTo'), stored)}
          </Group>
          <Group title={t('ctest.g.suggest')}>
            {check('suggest_cq_zone', t('ctest.f.sCq'), stored)}
            {check('suggest_itu_zone', t('ctest.f.sItu'), stored)}
            {check('suggest_domain', t('ctest.f.sDomain'), stored)}
            {check('suggest_dxcc_pfx', t('ctest.f.sDxcc'), stored)}
          </Group>
          <Group title={t('ctest.g.dupes')}>
            {check('dupe_no_band', t('ctest.f.noBand'), t('ctest.tip.noBand'))}
            {check('dupe_no_mode', t('ctest.f.noMode'), t('ctest.tip.noMode'))}
            {int('log_bar_mode', t('ctest.f.logBar'), stored)}
          </Group>
        </div>
        <footer>
          <span className="rtty-spacer" />
          <button type="button" className="glass sstv-btn" disabled={busy || c.name.trim() === ''} onClick={() => void save()}>
            {t('confirm.ok')}
          </button>
          <button type="button" className="glass sstv-btn" onClick={onClose}>
            {t('confirm.cancel')}
          </button>
        </footer>
      </div>
    </div>
  );
}
