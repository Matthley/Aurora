/**
 * Una macro premuta mentre si trasmette ricomincia da capo (scelta del proprietario): il testo non ancora partito della trasmissione
 * si butta e parte la macro nuova dall'inizio, senza togliere il PTT. Vale per la stessa macro e per un'altra: vince l'ultima
 * pressione, e solo i suoi `<RX>`/`<RXANDCLEAR>` contano. Qui le parti pure: la finestra di trasmissione dopo la ripartenza e l'ordine
 * delle richieste al motore.
 */

/** La finestra di trasmissione dopo la ripartenza: restano i primi `kept` caratteri (gia' passati al modulatore), poi la macro nuova. */
export function restartBox(box: string, kept: number, message: string): string {
  return box.slice(0, Math.max(0, Math.min(kept, box.length))) + message;
}

/**
 * Le pressioni delle macro in fila: le richieste partono una dopo l'altra nell'ordine dei tasti (mai due insieme, cosi' il motore le
 * riceve in ordine e l'ultima vince davvero), senza limite. `isLatest` dice se una pressione e' ancora l'ultima: solo l'ultima apre o
 * chiude la trasmissione e pulisce la finestra. `cancel` (Esc) rende vecchie tutte quelle fatte.
 */
export function createLatestSequencer() {
  let gen = 0;
  let chain: Promise<unknown> = Promise.resolve();
  return {
    run<T>(job: () => Promise<T>): { gen: number; done: Promise<T> } {
      const mine = ++gen;
      const done = chain.then(job, job);
      chain = done.catch(() => undefined);
      return { gen: mine, done };
    },
    isLatest: (g: number) => g === gen,
    cancel: () => {
      gen++;
    },
  };
}
