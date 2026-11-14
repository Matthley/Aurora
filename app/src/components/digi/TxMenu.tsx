import { useEffect, useRef } from 'react';
import { useT } from '../../i18n';
import { MenuList } from './MenuBar';

/** Il testo della finestra di trasmissione dopo Taglia o Incolla: `sel` e' la selezione nella casella (inizio, fine). */
export function cutText(text: string, sel: [number, number]): string {
  const [a, b] = [Math.min(sel[0], sel[1]), Math.max(sel[0], sel[1])];
  return text.slice(0, a) + text.slice(b);
}

export function pasteText(text: string, sel: [number, number], clip: string): string {
  const [a, b] = [Math.min(sel[0], sel[1]), Math.max(sel[0], sel[1])];
  return text.slice(0, a) + clip + text.slice(b);
}

interface Props {
  at: { x: number; y: number };
  box: HTMLTextAreaElement | null;
  text: string;
  /** Il testo nuovo della casella (passa dalla stessa strada della tastiera: in trasmissione parte subito). */
  onText(v: string): void;
  /** Pulisci tutto: la casella e lo storico di quello gia' trasmesso. */
  onClearAll(): void;
  onClose(): void;
}

/** Il menu del tasto destro nella finestra di trasmissione: Pulisci tutto, Taglia, Copia, Incolla. */
export function TxMenu({ at, box, text, onText, onClearAll, onClose }: Props) {
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  // la selezione si legge all'apertura: cliccando il menu la casella perde il fuoco
  const sel = useRef<[number, number]>([box?.selectionStart ?? text.length, box?.selectionEnd ?? text.length]);

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    document.addEventListener('mousedown', close);
    document.addEventListener('keydown', esc);
    return () => {
      document.removeEventListener('mousedown', close);
      document.removeEventListener('keydown', esc);
    };
  }, [onClose]);

  const [a, b] = sel.current;
  const selected = text.slice(Math.min(a, b), Math.max(a, b));
  const write = (v: string) => void navigator.clipboard?.writeText(v).catch(() => undefined);
  const left = Math.max(0, Math.min(at.x, window.innerWidth - 200));
  const top = Math.max(0, Math.min(at.y, window.innerHeight - 170));
  return (
    <div className="digi-menu" style={{ position: 'fixed', left, top, zIndex: 60 }} ref={ref} onContextMenu={(e) => e.preventDefault()} aria-label={t('digi.tx.title')}>
      <MenuList
        items={[
          { label: t('digi.txm.clearAll'), disabled: text === '', onSelect: onClearAll },
          'sep',
          {
            label: t('digi.txm.cut'),
            disabled: selected === '',
            onSelect: () => {
              write(selected);
              onText(cutText(text, sel.current));
            },
          },
          { label: t('digi.txm.copy'), disabled: text === '', onSelect: () => write(selected !== '' ? selected : text) },
          {
            label: t('digi.txm.paste'),
            onSelect: () =>
              void navigator.clipboard
                ?.readText()
                .then((clip) => {
                  if (clip) onText(pasteText(text, sel.current, clip));
                })
                .catch(() => undefined),
          },
        ]}
        onDone={onClose}
      />
    </div>
  );
}
