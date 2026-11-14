import { useState } from 'react';
import { useT } from '../i18n';
import { cssColor, MACRO_ROWS, textColorFor, type Macro } from '../lib/macroLang';
import type { MacroScope } from '../lib/digiMacroEdit';
import { browserStorage } from '../lib/theme';

/**
 * Le macro della pagina Impostazioni › Modi › PSK · RTTY · CW › Macro, divise nelle quattro righe della barra:
 * un gruppo apribile per riga (solo la prima ha i tasti F1-F12), dentro una riga per ognuno dei 12 posti.
 * I nomi salvati restano `F1`, `Ctrl-F1`, `Shift-F1`, `Ctrl-Shift-F1`: cambia solo come si mostrano.
 */

/** Chiave (sincronizzata col server, prefisso `aurora.`) dei gruppi aperti. */
export const OPEN_ROWS_KEY = 'aurora.sw.macroRows';

type Store = Pick<Storage, 'getItem' | 'setItem'> | null | undefined;

/** I gruppi aperti: di base solo la riga 1. */
export function readOpenRows(storage: Store): boolean[] {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(OPEN_ROWS_KEY) ?? 'null');
    if (Array.isArray(raw) && raw.length === MACRO_ROWS.length) return raw.map((v) => v === true);
  } catch {
    // valore rovinato: si torna al solito
  }
  return MACRO_ROWS.map((_, i) => i === 0);
}

export function writeOpenRows(storage: Store, open: boolean[]): void {
  try {
    storage?.setItem(OPEN_ROWS_KEY, JSON.stringify(open));
  } catch {
    // spazio pieno o archivio bloccato: lo stato vale solo per questa visita
  }
}

/** Il nome della macro al posto `pos` (1-12) della riga `row` (0-3). */
export function slotName(row: number, pos: number): string {
  return `${MACRO_ROWS[row] ?? ''}F${pos}`;
}

/** Cosa mostra il chip: «F1» nella riga 1 (tastiera), il solo numero nelle altre (si usano col clic). */
export function slotChip(row: number, pos: number): string {
  return row === 0 ? `F${pos}` : String(pos);
}

const isFilled = (m: Macro | undefined): m is Macro => m !== undefined && (m.label.trim() !== '' || m.text.trim() !== '');

export function MacroSlots({
  macros,
  scopeOf,
  modeName,
  onEdit,
}: {
  macros: Map<string, Macro>;
  scopeOf: (name: string) => MacroScope;
  modeName: string;
  onEdit: (name: string) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState<boolean[]>(() => readOpenRows(browserStorage()));
  const toggle = (row: number) => {
    const next = open.map((v, i) => (i === row ? !v : v));
    setOpen(next);
    writeOpenRows(browserStorage(), next);
  };
  const positions = Array.from({ length: 12 }, (_, i) => i + 1);
  return (
    <div className="sw-mrows">
      {MACRO_ROWS.map((_, row) => {
        const isOpen = open[row] === true;
        const filled = positions.filter((p) => isFilled(macros.get(slotName(row, p)))).length;
        const title = row === 0 ? t('sw.macros.row1') : t('sw.macros.rowN', { n: row + 1 });
        const bodyId = `sw-mrow-${row}`;
        return (
          <section key={row} className={`sw-mrow${isOpen ? ' sw-mrow--open' : ''}`} data-row={row + 1}>
            <button type="button" className="sw-mrow-head" aria-expanded={isOpen} aria-controls={bodyId} onClick={() => toggle(row)}>
              <span className="sw-mrow-chev" aria-hidden="true">
                ▸
              </span>
              <span className="sw-mrow-title">{title}</span>
              <span className="sw-mrow-count">{filled}/12</span>
            </button>
            {isOpen ? (
              <ul className="sw-mslots" id={bodyId}>
                {positions.map((pos) => {
                  const name = slotName(row, pos);
                  const m = macros.get(name);
                  const has = isFilled(m);
                  const bg = has ? cssColor(m.color) : undefined;
                  const scope = has ? scopeOf(name) : 'all';
                  return (
                    <li key={name} className={has ? undefined : 'sw-mslot--empty'} data-name={name}>
                      <b className="sw-mslot-key" style={bg && m ? { background: bg, color: textColorFor(m.color) } : undefined}>
                        {slotChip(row, pos)}
                      </b>
                      <span className="sw-mslot-label">{has ? m.label || '—' : t('sw.macros.free')}</span>
                      <code className="sw-mslot-text" title={has ? m.text : undefined}>
                        {has ? m.text.replace(/\s+/g, ' ') : ''}
                      </code>
                      <span className="sw-mslot-scope">{has ? (scope === 'mode' ? modeName : t('digi.edit.scopeAll')) : ''}</span>
                      <button type="button" className="glass" onClick={() => onEdit(name)}>
                        {has ? t('sw.macros.edit') : t('sw.macros.add')}
                      </button>
                    </li>
                  );
                })}
              </ul>
            ) : null}
          </section>
        );
      })}
    </div>
  );
}
