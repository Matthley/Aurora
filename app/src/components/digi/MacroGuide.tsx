import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import { useLang, useT, type Key } from '../../i18n';
import { pick } from '../../i18n/core';
import { HELP_CATEGORIES, MACRO_COMMANDS, MACRO_EVENTS, type HelpCategory, type MacroCommand, type MacroEvent } from '../../lib/macroCommands';

interface Props {
  onClose(): void;
}

/** Le voci della colonna di sinistra: le categorie dei comandi e, in fondo, le macro evento. */
type GuideCat = HelpCategory | 'events';
const CATS: readonly GuideCat[] = [...HELP_CATEGORIES, 'events'];

type Lang = Parameters<typeof pick>[1];

/** Il testo su cui cerca il filtro: comando, altri nomi, esempio e spiegazioni nella lingua scelta. */
function haystack(c: MacroCommand, lang: Lang): string {
  return [c.tag, ...(c.aliases ?? []), c.ex, pick(c.desc, lang), c.long ? pick(c.long, lang) : ''].join(' ').toLowerCase();
}

function eventHaystack(e: MacroEvent, lang: Lang): string {
  return `${e.name} ${pick(e.desc, lang)}`.toLowerCase();
}

/**
 * «Guida alle macro» (menu Aiuto della pagina dei modi testuali e finestra di modifica di una macro): a sinistra le categorie, ognuna
 * col suo colore e il numero di comandi; a destra i comandi della categoria scelta, uno per riga (comando, spiegazione breve, esempio),
 * che con un clic o Invio si aprono sulla spiegazione lunga. Il filtro in alto cerca in tutte le categorie.
 */
export function MacroGuide({ onClose }: Props) {
  const t = useT();
  const lang = useLang((s) => s.lang);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<GuideCat>('data');
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const search = useRef<HTMLInputElement>(null);
  const catRefs = useRef(new Map<GuideCat, HTMLButtonElement>());
  useEffect(() => {
    search.current?.focus();
  }, []);
  const q = query.trim().toLowerCase();
  const searching = q !== '';

  const byCat = useMemo(() => {
    const m = new Map<GuideCat, { cmds: MacroCommand[]; events: MacroEvent[] }>();
    for (const cat of CATS) m.set(cat, { cmds: [], events: [] });
    for (const c of MACRO_COMMANDS) if (q === '' || haystack(c, lang).includes(q)) m.get(c.cat)?.cmds.push(c);
    for (const e of MACRO_EVENTS) if (q === '' || eventHaystack(e, lang).includes(q)) m.get('events')?.events.push(e);
    return m;
  }, [q, lang]);
  const count = (cat: GuideCat) => {
    const g = byCat.get(cat);
    return g ? g.cmds.length + g.events.length : 0;
  };
  const listCats = CATS.filter((cat) => count(cat) > 0);
  // durante la ricerca la voce evidenziata e' la scelta se ha risultati, altrimenti la prima con risultati
  const current: GuideCat = listCats.includes(selected) ? selected : (listCats[0] ?? selected);
  const catTitle = (cat: GuideCat) => t(cat === 'events' ? 'mguide.events' : (`mguide.cat.${cat}` as Key));
  const catDesc = (cat: GuideCat) => t(`mguide.catd.${cat}` as Key);

  const toggle = (id: string) =>
    setOpen((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const choose = (cat: GuideCat) => {
    setSelected(cat);
    if (searching) document.getElementById(`mguide-${cat}`)?.scrollIntoView?.({ block: 'start' });
  };

  const onListKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const i = listCats.indexOf(current);
    let next: GuideCat | undefined;
    if (e.key === 'ArrowDown' || e.key === 'ArrowRight') next = listCats[Math.min(listCats.length - 1, i + 1)];
    else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') next = listCats[Math.max(0, i - 1)];
    else if (e.key === 'Home') next = listCats[0];
    else if (e.key === 'End') next = listCats[listCats.length - 1];
    if (next === undefined) return;
    e.preventDefault();
    choose(next);
    catRefs.current.get(next)?.focus();
  };

  const row = (id: string, tag: string, desc: string, off: boolean, more: { ex?: string; long?: string; aliases?: readonly string[] }) => {
    const expanded = open.has(id);
    const canOpen = Boolean(more.long || more.ex);
    return (
      <li key={id} className={`mguide-row${off ? ' mguide-row--off' : ''}${expanded ? ' mguide-row--open' : ''}`}>
        <button type="button" className="mguide-line" aria-expanded={canOpen ? expanded : undefined} disabled={!canOpen} onClick={() => toggle(id)}>
          <code className="mguide-tag">{tag}</code>
          <span className="mguide-desc">
            {desc}
            {more.aliases ? <span className="mguide-alias"> · {more.aliases.join(' · ')}</span> : null}
            {off ? <span className="mguide-badge">{t('mguide.notAvail')}</span> : null}
          </span>
          {more.ex ? <code className="mguide-chip">{more.ex}</code> : <span />}
        </button>
        {expanded ? (
          <div className="mguide-detail">
            {more.long ? <p>{more.long}</p> : null}
            {more.ex ? (
              <p>
                <span className="mguide-exlabel">{t('mguide.example')}</span> <code className="mguide-full">{more.ex}</code>
              </p>
            ) : null}
          </div>
        ) : null}
      </li>
    );
  };

  const section = (cat: GuideCat) => {
    const g = byCat.get(cat);
    if (!g) return null;
    return (
      <section key={cat} id={`mguide-${cat}`} className={`mguide-sec mguide-c-${cat}`} aria-labelledby={`mguide-h-${cat}`}>
        <header className="mguide-sechead">
          <h3 id={`mguide-h-${cat}`}>
            <span className="mguide-dot" aria-hidden="true" />
            {catTitle(cat)}
            <span className="mguide-count">{count(cat)}</span>
          </h3>
          <p>{catDesc(cat)}</p>
          {cat === 'events' ? <p>{t('mguide.eventsIntro')}</p> : null}
        </header>
        <ul className="mguide-rows">
          {g.cmds.map((c) => row(c.tag, c.tag, pick(c.desc, lang), Boolean(c.soon), { ex: c.ex, long: c.long ? pick(c.long, lang) : undefined, aliases: c.aliases }))}
          {g.events.map((e) => row(`ev:${e.name}`, e.name, pick(e.desc, lang), !e.runs, {}))}
        </ul>
      </section>
    );
  };

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
        {listCats.length === 0 ? (
          <p role="status">{t('mguide.none')}</p>
        ) : (
          <div className="mguide-cols">
            <div className="mguide-cats" role="tablist" aria-orientation="vertical" aria-label={t('mguide.toc')} onKeyDown={onListKey}>
              {listCats.map((cat) => {
                const on = cat === current;
                return (
                  <button
                    key={cat}
                    ref={(el) => {
                      if (el) catRefs.current.set(cat, el);
                      else catRefs.current.delete(cat);
                    }}
                    type="button"
                    role="tab"
                    aria-selected={on}
                    tabIndex={on ? 0 : -1}
                    className={`mguide-cat mguide-c-${cat}${on ? ' mguide-cat--on' : ''}`}
                    onClick={() => choose(cat)}
                  >
                    <span className="mguide-dot" aria-hidden="true" />
                    <span className="mguide-catname">{catTitle(cat)}</span>
                    <span className="mguide-count">{count(cat)}</span>
                  </button>
                );
              })}
            </div>
            <div className="mguide-body" role="tabpanel" aria-label={searching ? t('mguide.search') : catTitle(current)}>
              {(searching ? listCats : [current]).map(section)}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
