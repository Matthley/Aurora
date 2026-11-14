/** <TXE>: quanto si aspetta al massimo che cada il segnale ricevuto prima di trasmettere comunque. */
export const TXE_TIMEOUT_MS = 30_000;

/**
 * Un passo dell'attesa di <TXE>: si trasmette appena il segnale sulla frequenza di ricezione cade, o comunque passato `timeoutMs`
 * dall'inizio dell'attesa.
 */
export function txeStep(carrier: boolean | undefined, startedAt: number, now: number, timeoutMs = TXE_TIMEOUT_MS): 'wait' | 'send' {
  if (carrier !== true) return 'send';
  return now - startedAt >= timeoutMs ? 'send' : 'wait';
}
