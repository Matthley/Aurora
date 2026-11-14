import { useEffect, useMemo, useRef, useState } from 'react';
import { useLang, useT, type Key } from '../../i18n';
import { pick } from '../../i18n/core';
import { HELP_CATEGORIES, MACRO_COMMANDS, MACRO_EVENTS, type HelpCategory, type MacroCommand } from '../../lib/macroCommands';

interface Props {
  onClose(): void;
}

/** Il testo su cui cerca il filtro: comando, altri nomi, esempio e spiegazioni nella lingua scelta. */
function haystack(c: MacroCommand, lang: Parameters<typeof pick>[1]): string {
  return [c.tag, ...(c.aliases ?? []), c.ex, pick(c.desc, lang), c.long ? pick(c.long, lang) : ''].join(' ').toLowerCase();
}

/**
 * «Guida alle macro» (menu Aiuto della pagina dei modi testuali e finestra di modifica di una macro): tutti i comandi dell'elenco unico
 * `macroCommands.ts` per categoria, con sintassi, spiegazione ed esempio, e le macro evento. Il filtro in alto cerca in tutto.
 */
export function MacroGuide({ onClose }: Props) {
  const t = useT();
  const lang = useLang((s) => s.lang);
  const [query, setQuery] = useState('');
  const search = useRef<HTMLInputElement>(null);
  useEffect(() => {
    search.current?.focus();
  }, []);
  const q = query.trim().toLowerCase();
  const groups = useMemo(() => {
    const hit = MACRO_COMMANDS.filter((c) => q === '' || haystack(c, lang).includes(q));
    return HELP_CATEGORIES.map((cat) => ({ cat, items: hit.filter((c) => c.cat === cat) })).filter((g) => g.items.length > 0);
  }, [q, lang]);
  const events = MACRO_EVENTS.filter((e) => q === '' || `${e.name} ${pick(e.desc, lang)}`.toLowerCase().includes(q));
  const catTitle = (cat: HelpCategory) => t(`mguide.cat.${cat}` as Key);

  return (
    <div
      className="digi-modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="mguide-title"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          onClose();
        }
      }}
    >
      <div className="digi-dialog mguide">
        <header className="mguide-head">
          <strong id="mguide-title">{t('mguide.title')}</strong>
          <button type="button" className="glass sstv-btn" onClick={onClose}>
            {t('mguide.close')}
          </button>
        </header>
        <p className="mguide-intro">{t('mguide.intro')}</p>
        <label className="mguide-search">
          <span>{t('mguide.search')}</span>
          <input ref={search} type="search" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="<RX>, CQ, rotor…" />
        </label>
        {groups.length > 0 ? (
          <nav className="mguide-toc" aria-label={t('mguide.toc')}>
            {groups.map((g) => (
              <a key={g.cat} href={`#mguide-${g.cat}`} onClick={(e) => { e.preventDefault(); document.getElementById(`mguide-${g.cat}`)?.scrollIntoView({ block: 'start' }); }}>
                {catTitle(g.cat)}
              </a>
            ))}
          </nav>
        ) : null}
        <div className="mguide-body">
          {groups.map((g) => (
            <section key={g.cat} id={`mguide-${g.cat}`} aria-labelledby={`mguide-h-${g.cat}`}>
              <h3 id={`mguide-h-${g.cat}`}>{catTitle(g.cat)}</h3>
              <dl>
                {g.items.map((c) => (
                  <div key={c.tag} className={`mguide-item${c.soon ? ' mguide-item--off' : ''}`}>
                    <dt>
                      <code>{c.tag}</code>
                      {c.aliases ? <span className="mguide-alias"> · {c.aliases.map((a) => <code key={a}>{a}</code>)}</span> : null}
                      {c.soon ? <span className="mguide-badge">{t('mguide.notAvail')}</span> : null}
                    </dt>
                    <dd>
                      <p>{pick(c.desc, lang)}</p>
                      {c.long ? <p>{pick(c.long, lang)}</p> : null}
                      <p className="mguide-ex">
                        <span>{t('mguide.example')}</span> <code>{c.ex}</code>
                      </p>
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          ))}
          {events.length > 0 ? (
            <section id="mguide-events" aria-labelledby="mguide-h-events">
              <h3 id="mguide-h-events">{t('mguide.events')}</h3>
              <p>{t('mguide.eventsIntro')}</p>
              <dl>
                {events.map((e) => (
                  <div key={e.name} className={`mguide-item${e.runs ? '' : ' mguide-item--off'}`}>
                    <dt>
                      <code>{e.name}</code>
                      {e.runs ? null : <span className="mguide-badge">{t('mguide.notAvail')}</span>}
                    </dt>
                    <dd>
                      <p>{pick(e.desc, lang)}</p>
                    </dd>
                  </div>
                ))}
              </dl>
            </section>
          ) : null}
          {groups.length === 0 && events.length === 0 ? <p role="status">{t('mguide.none')}</p> : null}
        </div>
      </div>
    </div>
  );
}
