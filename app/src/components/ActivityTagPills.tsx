import { useT, type Key } from '../i18n';
import { CONTINENTS, CQ_TAG_LIST, STATE_TAG_LIST, useFilters } from '../lib/filters';
import { ensureHighlightFor } from '../lib/highlightFor';

/**
 * Fila di pillole del pulsantino «Filtro» di Attività di banda (come GridTracker): si sceglie quali novità vedere,
 * logica «o»; «Tutte» toglie il filtro. La scelta e' quella dei filtri (si ricorda e va anche alle Automazioni).
 */
export function ActivityTagPills() {
  const t = useT();
  const tags = useFilters((s) => s.filters.tags);
  const f = useFilters((s) => s.filters);
  const set = useFilters.getState().set;
  const toggle = (tag: string) => {
    const on = !tags.includes(tag);
    set({ tags: on ? [...tags, tag] : tags.filter((x) => x !== tag) });
    if (on) void ensureHighlightFor(tag);
  };
  const toggleCont = (code: string) =>
    set({ hideContinents: f.hideContinents.includes(code) ? f.hideContinents.filter((c) => c !== code) : [...f.hideContinents, code] });
  const list = (text: string, upper: boolean) => text.split(',').map((x) => (upper ? x.trim().toUpperCase() : x.trim())).filter(Boolean);
  return (
    <div className="bfl-pills" role="group" aria-label={t('bfl.title')}>
      <button type="button" className={tags.length === 0 ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={tags.length === 0} onClick={() => set({ tags: [] })}>
        {t('bfl.all')}
      </button>
      {[...CQ_TAG_LIST, ...STATE_TAG_LIST].map((tag) => (
        <button key={tag} type="button" className={tags.includes(tag) ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={tags.includes(tag)} onClick={() => toggle(tag)}>
          {t(`kind.${tag}` as Key)}
        </button>
      ))}
      <span className="bfl-sep" aria-hidden="true" />
      <span className="bfl-group">{t('bfl.except')}</span>
      {/* le «eccezioni» come in GridTracker (richiesta di ON7KEC): restringono quello che resta */}
      <button type="button" className={f.only === 'cq' ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={f.only === 'cq'} onClick={() => set({ only: f.only === 'cq' ? 'all' : 'cq' })}>
        {t('bfl.cqOnly')}
      </button>
      <button type="button" className={f.only === 'cq_rr73' ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={f.only === 'cq_rr73'} onClick={() => set({ only: f.only === 'cq_rr73' ? 'all' : 'cq_rr73' })}>
        {t('bfl.rr73AsCq')}
      </button>
      <button type="button" className={f.hasGrid ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={f.hasGrid} onClick={() => set({ hasGrid: !f.hasGrid })}>
        {t('bfl.hasGrid')}
      </button>
      <button type="button" className={f.hideOwnContinent ? 'bfl-pill bfl-pill--on' : 'bfl-pill'} aria-pressed={f.hideOwnContinent} onClick={() => set({ hideOwnContinent: !f.hideOwnContinent })}>
        {t('bfl.notMyCont')}
      </button>
      <span className="bfl-break" aria-hidden="true" />
      {/* continenti, paesi e nominativi da nascondere (gli stessi filtri di Impostazioni > Filtri; valgono anche per le Automazioni) */}
      <span className="bfl-group" title={t('fil.continents')}>{t('bfl.hide')}</span>
      {CONTINENTS.map((code) => (
        <button key={code} type="button" className={f.hideContinents.includes(code) ? 'bfl-pill bfl-pill--hidden' : 'bfl-pill'} aria-pressed={f.hideContinents.includes(code)} title={`${t('fil.continents')}: ${t(`cont.${code}` as Key)}`} onClick={() => toggleCont(code)}>
          {code}
        </button>
      ))}
      <input type="text" className="bfl-input" defaultValue={f.hideCountries.join(', ')} key={`c-${f.hideCountries.join(',')}`} placeholder={t('bfl.hideCountries')} aria-label={t('fil.countries')} title={t('fil.countries')} onBlur={(e) => set({ hideCountries: list(e.target.value, false) })} onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); }} />
      <input type="text" className="bfl-input" defaultValue={f.hideCalls.join(', ')} key={`k-${f.hideCalls.join(',')}`} placeholder={t('bfl.hideCalls')} aria-label={t('fil.calls')} title={t('fil.calls')} onBlur={(e) => set({ hideCalls: list(e.target.value, true) })} onKeyDown={(e) => { if (e.key === 'Enter') e.currentTarget.blur(); }} />
    </div>
  );
}
