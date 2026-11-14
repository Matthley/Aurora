import { useRef, useState } from 'react';
import { useT } from '../../i18n';
import { cssColor, MACRO_ROWS, type Macro } from '../../lib/macroLang';


interface Props {
  macros: ReadonlyMap<string, Macro>;
  /** Tutte e 4 le righe sempre in vista. */
  extended: boolean;
  disabled?: boolean;
  onToggleExtended(): void;
  onRun(macro: Macro): void;
  /** Clic destro: si modifica la macro di quel tasto (esista o no). */
  onEdit(name: string): void;
  /** Stato del CQ automatico: i tasti che lo comandano (<AUTOCQ>, <ACQ>) si vedono accesi o in pausa. */
  autoCq?: 'off' | 'on' | 'paused';
  /** Trascinamento (clic tenuto) o Alt+freccia: i due tasti si scambiano le macro. */
  onMove?(from: string, to: string): void;
}

/** Quanto tenere premuto un tasto prima di poterlo spostare (ms). */
export const HOLD_MS = 380;

/** Il tasto vicino nella griglia 4×12 (Alt+freccia), o `null` al bordo. */
export function neighborSlot(name: string, dir: 'left' | 'right' | 'up' | 'down'): string | null {
  const m = /^(Ctrl-)?(Shift-)?F(\d+)$/.exec(name);
  if (!m) return null;
  let row = MACRO_ROWS.indexOf(`${m[1] ?? ''}${m[2] ?? ''}` as (typeof MACRO_ROWS)[number]);
  let col = Number(m[3]) - 1;
  if (dir === 'left') col--;
  else if (dir === 'right') col++;
  else if (dir === 'up') row--;
  else row++;
  if (row < 0 || row > 3 || col < 0 || col > 11) return null;
  return `${MACRO_ROWS[row]}F${col + 1}`;
}

const ARROWS: Record<string, 'left' | 'right' | 'up' | 'down'> = { ArrowLeft: 'left', ArrowRight: 'right', ArrowUp: 'up', ArrowDown: 'down' };

/** Il tasto di una macro in breve, per l'angolo del pulsante: «F1», «C-F1» (Ctrl), «S-F1» (Shift), «CS-F1» (Ctrl+Shift). */
export function shortKey(name: string): string {
  const m = /^(Ctrl-)?(Shift-)?(F\d+)$/.exec(name);
  if (!m) return name;
  const mods = `${m[1] ? 'C' : ''}${m[2] ? 'S' : ''}`;
  return mods ? `${mods}-${m[3]}` : (m[3] ?? name);
}

/** Vero se la macro accende o spegne il CQ automatico. */
const isAutoCqMacro = (text: string) => /<\s*(AUTOCQ|ACQ)\s*>/i.test(text);


/**
 * La barra dei comandi: 12 tasti per riga a gruppi di 4 (F1-F4 · F5-F8 · F9-F12), 4 righe (nessun modificatore,
 * Ctrl, Shift, Ctrl+Shift). Ogni pulsante ha etichetta e colore; clic o tasto eseguono la macro, clic destro la modificano.
 */
export function ControlBar({ macros, extended, disabled = false, onToggleExtended, onRun, onEdit, autoCq = 'off', onMove }: Props) {
  const t = useT();
  // clic tenuto: dopo HOLD_MS il tasto si accende (bordo) e si puo' trascinare su un altro; un clic breve esegue come sempre
  const hold = useRef<{ name: string; x: number; y: number; timer: number | null } | null>(null);
  const [armed, setArmed] = useState<string | null>(null);
  const [over, setOver] = useState<string | null>(null);
  const suppressClick = useRef(false);
  const toolbar = useRef<HTMLDivElement>(null);
  const clearHold = () => {
    if (hold.current?.timer != null) window.clearTimeout(hold.current.timer);
    hold.current = null;
    setArmed(null);
    setOver(null);
  };
  const slotAt = (x: number, y: number): string | null => {
    const el = typeof document.elementFromPoint === 'function' ? document.elementFromPoint(x, y) : null;
    return el?.closest<HTMLElement>('[data-macro]')?.dataset.macro ?? null;
  };
  // le 4 righe di macro restano tutte visibili; solo la prima ha i tasti F1-F12, le altre si usano col clic (proprietario, 2026-11-14)
  void extended;
  void onToggleExtended;
  const rows = [0, 1, 2, 3];

  return (
    <div ref={toolbar} className={`digi-bar-keys${armed ? ' digi-bar-keys--dragging' : ''}`} role="toolbar" aria-label={t('digi.bar.title')}>
      <div className="digi-keys">
        {rows.map((r) => (
          <div key={r} className="digi-keyrow">
            {Array.from({ length: 12 }, (_, i) => {
              const name = `${MACRO_ROWS[r]}F${i + 1}`;
              const macro = macros.get(name);
              const blank = !macro || (macro.label.trim() === '' && macro.text.trim() === '');
              const acq = !blank && autoCq !== 'off' && isAutoCqMacro(macro?.text ?? '') ? ` digi-key--acq-${autoCq}` : '';
              return (
                <button
                  key={name}
                  type="button"
                  data-macro={name}
                  className={`digi-key${armed === name ? ' digi-key--armed' : ''}${over === name && armed !== name ? ' digi-key--drop' : ''}${blank ? ' digi-key--blank' : ''}${i % 4 === 0 && i > 0 ? ' digi-key--gap' : ''}${acq}`}
                  aria-pressed={acq ? true : undefined}
                  style={
                    !blank && macro?.color != null
                      ? {
                          // il colore della macro tinge lo sfondo e il bordo; il testo e' bianco col tema scuro e nero col chiaro
                          background: `color-mix(in srgb, ${cssColor(macro.color)} 26%, var(--key-base))`,
                          borderColor: `color-mix(in srgb, ${cssColor(macro.color)} 70%, transparent)`,
                        }
                      : undefined
                  }
                  title={blank ? `${name} — ${t('digi.bar.emptyHint')}` : `${name} — ${macro?.text ?? ''}${onMove ? `
${t('digi.bar.moveHint')}` : ''}`}
                  disabled={disabled && !blank}
                  onPointerDown={(e) => {
                    if (!onMove || blank || e.button !== 0) return;
                    const timer = window.setTimeout(() => {
                      if (hold.current) hold.current.timer = null;
                      setArmed(name);
                    }, HOLD_MS);
                    hold.current = { name, x: e.clientX, y: e.clientY, timer };
                    try {
                      e.currentTarget.setPointerCapture(e.pointerId);
                    } catch {
                      // senza cattura si trascina lo stesso finche' il puntatore resta sulla barra
                    }
                  }}
                  onPointerMove={(e) => {
                    const h = hold.current;
                    if (!h) return;
                    if (h.timer !== null) {
                      // mosso prima della soglia: niente spostamento, resta un clic normale
                      if (Math.hypot(e.clientX - h.x, e.clientY - h.y) > 8) clearHold();
                      return;
                    }
                    setOver(slotAt(e.clientX, e.clientY));
                  }}
                  onPointerUp={(e) => {
                    const h = hold.current;
                    if (!h) return;
                    if (h.timer === null) {
                      // dopo la soglia il rilascio non esegue mai la macro; sposta solo se si e' su un altro tasto
                      suppressClick.current = true;
                      const to = slotAt(e.clientX, e.clientY);
                      if (to && to !== h.name) onMove?.(h.name, to);
                    }
                    clearHold();
                  }}
                  onPointerCancel={clearHold}
                  onKeyDown={(e) => {
                    const dir = ARROWS[e.key];
                    if (!onMove || !e.altKey || !dir || blank) return;
                    e.preventDefault();
                    const to = neighborSlot(name, dir);
                    if (!to) return;
                    onMove(name, to);
                    toolbar.current?.querySelector<HTMLElement>(`[data-macro="${to}"]`)?.focus();
                  }}
                  onClick={() => {
                    if (suppressClick.current) {
                      suppressClick.current = false;
                      return;
                    }
                    if (macro && !blank) onRun(macro);
                  }}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    // al tocco il clic tenuto apre anche il menu: mentre si sposta non si apre l'editor
                    if (armed || suppressClick.current) return;
                    onEdit(name);
                  }}
                >
                  <span className="digi-key-label">{blank ? '' : macro?.label}</span>
                  {r === 0 ? <span className="digi-key-hint" aria-hidden="true">{shortKey(name)}</span> : null}
                </button>
              );
            })}
          </div>
        ))}
      </div>
    </div>
  );
}
