/**
 * L'ordine di una macro: cosa fare della trasmissione, della ricezione e della finestra di trasmissione secondo DOVE stanno i comandi
 * nello script. `<RX>` e `<RXC>` sono segni dentro il testo: il testo prima parte, a quel punto si torna in ricezione, il testo dopo
 * resta nella finestra senza partire. `<CLEARTXWINDOW>` pulisce quello che c'e' fino a quel punto (la casella e lo storico), il testo
 * dopo resta. Gli altri comandi (modo, rotore, log...) si eseguono subito nell'ordine in cui compaiono.
 */
import { tidyText, type MacroAction, type MacroStep } from './macroLang';

export interface MacroPlan {
  /** I comandi da eseguire subito, nell'ordine dello script (tutto tranne TX/RX e la pulizia della finestra di trasmissione). */
  actions: MacroAction[];
  /** Prima del testo si svuota la finestra di trasmissione (casella e storico): c'era un `<CLEARTXWINDOW>` prima del testo. */
  clearBox: boolean;
  /** Il testo che parte (o che va nella casella se non si trasmette): quello prima del primo `<RX>`/`<RXC>`. */
  text: string;
  /** Il testo dopo `<RX>`/`<RXC>`: resta nella finestra di trasmissione, non parte. */
  after: string;
  /** `start` apre la trasmissione, `stop` la chiude a fine testo (`<TXTOGGLE>` mentre si trasmette). */
  tx: 'none' | 'start' | 'stop';
  /** `<TXE>`: prima di trasmettere si aspetta che cada il segnale sulla frequenza di ricezione. */
  waitCarrier: boolean;
  /** Finito il testo si torna in ricezione (`<RX>`, `<RXC>`, `<RXE>`). */
  rxAfter: boolean;
  /** Tornati in ricezione si pulisce la finestra di trasmissione (`<RXC>`, o `<CLEARTXWINDOW>` dopo un `<RX>`). */
  clearAfter: boolean;
}

/**
 * Il piano di una macro. `sending`: la trasmissione e' gia' in corso (allora il testo si accoda a quello che sta partendo).
 * Se la macro non trasmette e non si sta trasmettendo, `<RX>` non ha niente da chiudere: il testo resta tutto nella casella e un
 * `<CLEARTXWINDOW>` (o `<RXC>`) pulisce quello che c'era prima.
 */
export function planMacro(steps: readonly MacroStep[], sending: boolean): MacroPlan {
  const transmits = sending || steps.some((s) => 'action' in s && (s.action.type === 'tx' || s.action.type === 'txe' || s.action.type === 'txToggle'));
  const actions: MacroAction[] = [];
  let pre = '';
  let post = '';
  let afterRx = false;
  let clearBox = false;
  let clearAfter = false;
  let rxAfter = false;
  let waitCarrier = false;
  let tx: MacroPlan['tx'] = 'none';
  const clearHere = () => {
    if (afterRx) {
      post = '';
      clearAfter = true;
    } else {
      pre = '';
      clearBox = true;
    }
  };
  for (const step of steps) {
    if ('text' in step) {
      if (afterRx) post += step.text;
      else pre += step.text;
      continue;
    }
    const a = step.action;
    switch (a.type) {
      case 'tx': tx = 'start'; break;
      case 'txe': tx = 'start'; waitCarrier = true; break;
      case 'txToggle': tx = sending ? 'stop' : 'start'; break;
      case 'rx':
        if (!transmits) {
          // niente da chiudere: <RXC> pulisce comunque la finestra
          if (a.clear) clearHere();
          break;
        }
        rxAfter = true;
        if (a.clear) clearAfter = true;
        afterRx = true;
        break;
      case 'rxe': if (transmits) rxAfter = true; break;
      case 'clearTx': clearHere(); break;
      case 'clearWindow': clearHere(); actions.push({ type: 'clearRx' }); break;
      default: actions.push(a);
    }
  }
  return { actions, clearBox, text: clean(pre), after: clean(post), tx, waitCarrier, rxAfter, clearAfter };
}

const clean = (s: string) => tidyText(s).replace(/^[\r\n\t ]+|[\r\n\t ]+$/g, '');

/**
 * `<AUTOCQ>` / `<ASAUTOCQ>` seguito da un testo: quel testo (con i suoi comandi, che si riempiono a ogni giro) diventa la chiamata
 * dell'Auto CQ, che si accende. Restituisce lo script da eseguire subito (quello prima del comando) e la chiamata; `cq` e' `null` se
 * dopo il comando non c'e' niente (allora vale il comando da solo: `<AUTOCQ>` accende o spegne).
 */
export function splitAutoCq(script: string): { before: string; cq: string | null } {
  const m = /<(?:AS)?AUTOCQ>|<ACQ>/.exec(script);
  if (!m) return { before: script, cq: null };
  const rest = script.slice(m.index + m[0].length);
  if (rest.trim() === '') return { before: script, cq: null };
  return { before: script.slice(0, m.index), cq: rest.trim() };
}
