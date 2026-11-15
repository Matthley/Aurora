# WinKeyer

Supporto ai manipolatori CW esterni K1EL WinKeyer 2 e 3 e ai compatibili (WKUSB, WKmini). Il WinKeyer riceve il testo in ASCII, genera la
CW con i tempi giusti, comanda PTT e tasto della radio e accetta il paddle.

## Dove sta

- Impostazioni › Stazione › **WinKeyer** (`app/src/settings/ModePages.tsx`, `WinKeyerPage`; voce `winkeyer` in `lib/settingsTree.ts`).
- Nel dialogo del modo CW della pagina dei modi testuali: uscita CW (audio o WinKeyer), stato e «Prova».
- Core: `core/crates/rig-control/src/winkeyer.rs` (protocollo, porta seriale, WinKeyer finto per le prove) e
  `core/crates/mode-digi/src/service/keyer.rs` (trasmissione con il WinKeyer). Rotta `POST /api/v1/digi/winkeyer/test`; lo stato
  `keyer` (collegato, porta, firmware, potenziometro) e' nello stato di `GET /api/v1/digi`.

## Impostazioni (`settings.winkeyer` di `digi.json`)

| Riquadro | Voce | Campo | Limiti, valore di base | Comando |
|---|---|---|---|---|
| Collegamento | Uscita CW | `cw_output` | `audio` / `winkeyer` | — |
| Collegamento | Porta | `port` | COMn, 1200 baud 8N2 | — |
| Collegamento | Stato e firmware | (sola lettura) | 23 = WK2.3, 31 = WK3.1 | `00 02` risposta |
| Velocita' | WPM | `cw_wpm` (del CW) | 5-60 | `02 nn` |
| Velocita' | Dal potenziometro | `use_pot` | spento | `02 00` |
| Velocita' | Potenziometro minimo / ampiezza | `pot_min_wpm`, `pot_range_wpm` | 5-60 (10), 5-40 (25) | `05 min ampiezza 00` |
| Velocita' | Farnsworth | `farnsworth_wpm` | 0 = spento, 10-99 | `0D nn` |
| Manipolazione | Peso | `weight` | 10-90 (50) | `03 nn` |
| Manipolazione | Rapporto punto/linea | `ratio` | 33-66 (50 = 1:3) | `17 nn` |
| Manipolazione | Compensazione | `key_comp_ms` | 0-250 ms | `11 nn` |
| Manipolazione | First extension | `first_extension_ms` | 0-250 ms | `10 nn` |
| Manipolazione | Autospace | `autospace` | spento | `0E` bit 1 |
| Manipolazione | Contest spacing | `contest_spacing` | spento | `0E` bit 0 |
| PTT | Il WinKeyer comanda il PTT | `ptt` | acceso | `09` bit 0 |
| PTT | Anticipo / coda | `lead_ms`, `tail_ms` | 0-2500 ms a passi di 10 (50, 100) | `04 lead tail` |
| PTT | Hang time del paddle | `hang_time` | 0-3 = 1 / 1,33 / 1,67 / 2 spazi di parola | `09` bit 5-4 |
| Paddle e sidetone | Modo paddle | `paddle` | `iambic_b`, `iambic_a`, `ultimatic`, `bug` | `0E` bit 5-4 |
| Paddle e sidetone | Paddle invertito | `paddle_swap` | spento | `0E` bit 3 |
| Paddle e sidetone | Il paddle ha la precedenza | `paddle_breakin` | acceso | stato bit 1 → `0A` |
| Paddle e sidetone | Sidetone / nota | `sidetone`, `sidetone_hz` | acceso, 400-4000 Hz (800) | `09` bit 1, `01 n` (4000/n Hz) |

Il registro di modo (`0E`) ha sempre l'eco dei caratteri (bit 2): serve per l'avanzamento. Nel pin config (`09`) le due uscite del tasto
(bit 2 e 3) sono sempre accese.

## Protocollo usato

1. Apertura: tre `13` (comando nullo, rimette in riga), prova d'eco `00 04 55` (deve tornare `55`), host open `00 02` (torna la versione),
   poi le impostazioni della tabella e `15` (stato).
2. Testo: i caratteri vanno nel buffer in maiuscolo; `<AR> <BT> <KN> <SK>` (e `<AS> <BK> <SN>`) diventano `1B a b` (lettere unite); a capo
   diventa spazio; i caratteri che il WinKeyer non conosce si tolgono. Al massimo 12 caratteri in attesa d'eco, e niente se lo stato dice
   buffer quasi pieno.
3. Dal WinKeyer: `C0-FF` stato (bit 0 buffer quasi pieno, 1 break-in del paddle, 2 occupato, 3 tasto giu'), `80-BF` potenziometro, il resto
   eco. Un pezzo di testo e' partito quando sono tornati tutti i suoi caratteri; se l'eco manca, quando il WinKeyer non e' piu' occupato da
   400 ms.
4. Esc, macro che riparte, paddle toccato (con la precedenza accesa): `0A` (svuota il buffer). Chiusura: `0A`, `00 03`.

Il WinKeyer si apre alla prima trasmissione CW con uscita WinKeyer (o con «Prova») e resta aperto; si chiude passando all'audio o a un altro
modo, e si riapre se cambia la porta. La porta seriale e' solo per Windows (`#[cfg(windows)]`); su Linux l'apertura risponde con un errore.

## Prove

`cargo test -p rig-control --lib winkeyer` (handshake, invio, eco, svuotamento, velocita', potenziometro, codifica, chiusura) e
`cargo test -p mode-digi --lib service::keyer` (macro, Esc, macro che riparte, paddle, cambio di velocita', «Prova» e porta mancante), con
il WinKeyer finto di `rig_control::winkeyer::fake`. Le prove con un WinKeyer vero le fa il proprietario.
