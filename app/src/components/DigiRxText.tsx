import { memo, useEffect, useSyncExternalStore, type RefObject } from 'react';
import type React from 'react';
import { getDigiBase, getDigiText, getDigiWeak, onDigiText } from '../lib/digiBus';
import { rxPieces } from '../lib/rxSearch';

/**
 * Il testo della finestra di ricezione dei modi testuali: le righe trasmesse (iniziano con «» ») sottolineate se richiesto, i caratteri
 * incerti (RTTY: probabilita' sotto 0,9, `weak` con '1' alla stessa posizione) in chiaro.
 * `base` e' la posizione assoluta del primo carattere (vedi `getDigiBase`): ogni riga ha per chiave la sua posizione assoluta, cosi' a ogni
 * carattere nuovo si ridisegna solo l'ultima riga e non tutte le centinaia di righe della finestra.
 */
export function DigiRxText({ text, weak, underlineSent, showSent = true, base = 0, search = '' }: { text: string; weak: string; underlineSent: boolean; /** Spento: le righe trasmesse non si mostrano (restano nella finestra di trasmissione). */ showSent?: boolean; base?: number; /** Parola cercata: le occorrenze si evidenziano con <mark>. */ search?: string }): React.ReactNode {
  let offset = 0;
  return text.split('\n').map((line, i, all) => {
    const start = offset;
    offset += line.length + 1;
    if (!showSent && line.startsWith('»')) return null;
    return <RxLine key={base + start} line={line} mask={weak.slice(start, start + line.length)} newline={i < all.length - 1} underline={underlineSent && line.startsWith('» ')} search={search} />;
  });
}

/** Una riga della finestra: si ridisegna solo se cambia il suo testo, i suoi caratteri incerti o il suo stile. */
const RxLine = memo(function RxLine({ line, mask, newline, underline, search }: { line: string; mask: string; newline: boolean; underline: boolean; search: string }) {
  const body = search.trim() === '' ? withWeak(line, mask) : withSearch(line, mask, search);
  const nl = newline ? '\n' : '';
  return underline ? (
    <u className="digi-sent">
      {body}
      {nl}
    </u>
  ) : (
    <span>
      {body}
      {nl}
    </span>
  );
});

/**
 * La finestra di ricezione collegata al testo dei modi testuali: si iscrive da sola, cosi' a ogni carattere ricevuto si ridisegna solo lei e
 * non tutta la pagina; a ogni carattere nuovo `box` scorre in fondo.
 */
export function DigiRxLive({ box, underlineSent, showSent, paused = false, search = '' }: { box: RefObject<HTMLElement | null>; underlineSent: boolean; showSent: boolean; /** Vista ferma: il testo continua ad arrivare ma la finestra non scorre in fondo. */ paused?: boolean; search?: string }): React.ReactNode {
  const text = useSyncExternalStore(onDigiText, getDigiText);
  const weak = useSyncExternalStore(onDigiText, getDigiWeak);
  const base = useSyncExternalStore(onDigiText, getDigiBase);
  useEffect(() => {
    const el = box.current;
    if (el && !paused) el.scrollTop = el.scrollHeight;
  }, [text, box, paused]);
  return text === '' ? null : <DigiRxText text={text} weak={weak} base={base} underlineSent={underlineSent} showSent={showSent} search={search} />;
}

/** Una riga divisa in tratti certi e incerti. */
function withWeak(line: string, mask: string): React.ReactNode {
  if (!mask.includes('1')) return line;
  const parts: React.ReactNode[] = [];
  let start = 0;
  for (let k = 1; k <= line.length; k++) {
    if (k === line.length || mask[k] !== mask[start]) {
      const piece = line.slice(start, k);
      parts.push(mask[start] === '1' ? <span key={start} className="digi-weak">{piece}</span> : piece);
      start = k;
    }
  }
  return parts;
}

/** Una riga con le parole cercate evidenziate (il primo tratto di ogni occorrenza ha `data-first` per saltare da una all'altra). */
function withSearch(line: string, mask: string, search: string): React.ReactNode {
  return rxPieces(line, mask, search).map((p) => {
    const cls = p.weak ? 'digi-weak' : undefined;
    if (p.hit) return <mark key={p.start} className={`digi-hit${p.weak ? ' digi-weak' : ''}`} data-first={p.first ? '' : undefined}>{p.text}</mark>;
    return cls ? <span key={p.start} className={cls}>{p.text}</span> : p.text;
  });
}
