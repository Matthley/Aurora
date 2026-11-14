# Verifica delle macro dei modi testuali (CW, RTTY, PSK)

Verifica di ogni comando del catalogo `app/src/lib/macroCommands.ts`: lettura (`macroLang.ts`: `textTag`, `actionOf`, `runMacro`), ordine
(`macroPlan.ts`: `planMacro`, `splitAutoCq`) ed effetto vero nella pagina (`DigiPage.tsx`: `execute`, `run`, `apply`). Il comportamento
atteso viene dall'analisi del programma di riferimento per le macro (documentazione interna, non nel repository).

Esito: **OK** = funziona come atteso; **CORRETTO** = non funzionava o funzionava a metà, sistemato con questa verifica; **PARZIALE** =
funziona con un limite dichiarato; **NON DISPONIBILE** = Aurora non lo può fare, il catalogo e la Guida lo dicono e la pagina avvisa.

## Riepilogo

Conteggio sulle 102 voci del catalogo (una voce può raccogliere più forme, per esempio `<AFCON>`, `<AFCOFF>`, `<AFC>`):

| Esito | Voci |
|---|---|
| OK | 64 (7 di queste mancavano nel catalogo e sono state aggiunte) |
| CORRETTO | 16 |
| PARZIALE | 5 |
| NON DISPONIBILE | 17 |

Macro evento: 7 partono (OnStart, OnEnd, OnTX, OnCR, OnClickAny, Gray+, AutoCqText; le prime cinque non partivano), 4 non hanno un
evento in Aurora (OnAlarm, OnSP0, OnSP1, Hello).

## Come si esegue una macro (dopo la correzione)

1. `execute`: legge i file di `<TEXTFILE:...>` e l'ultimo QSO per `<ANYQSOBEFORE...>`; se c'è `<AUTOCQ>`/`<ASAUTOCQ>` seguito da testo,
   quel testo diventa la chiamata dell'Auto CQ e si esegue subito solo quello che sta prima.
2. `runMacro` riempie i comandi di testo e restituisce i pezzi **nell'ordine** (`steps`).
3. `planMacro` decide secondo la posizione: il testo prima del primo `<RX>`/`<RXC>` parte, quello dopo resta nella finestra di
   trasmissione; `<CLEARTXWINDOW>` pulisce fino al punto in cui sta; gli altri comandi si eseguono subito nell'ordine scritto.
4. `run` applica: comandi, pulizia, testo (trasmesso, accodato alla trasmissione in corso o messo nella casella), ritorno in ricezione e
   pulizia a fine trasmissione. Lo `switch` di `apply` è esaustivo (il compilatore segnala un'azione dimenticata).

## Difetti trovati e corretti

| # | Difetto | Correzione |
|---|---|---|
| 1 | Tutto il testo della macro partiva anche dopo `<RX>`: la posizione di `<RX>`/`<RXC>` non contava | `<RX>` è un segno nel testo: il testo dopo resta nella finestra senza partire |
| 2 | `<CLEARTXWINDOW>` dopo il testo non lo cancellava (il testo andava comunque nella casella) | pulisce fino al punto in cui sta; dopo `<RX>` pulisce tornati in ricezione |
| 3 | `<CLEARTXWINDOW><TX>` da solo trasmetteva il vecchio contenuto della casella | con la casella appena pulita non parte niente |
| 4 | Una macro senza `<TX>` premuta durante una trasmissione aperta metteva il testo nella casella, dove restava fermo | il testo si accoda alla trasmissione in corso |
| 5 | `<TXTOGGLE>` apriva sempre la trasmissione | in trasmissione la chiude a fine testo |
| 6 | `<SAVEQSO>` era sempre eseguito per ultimo, qualunque fosse l'ordine | si esegue nell'ordine scritto (come gli altri comandi) |
| 7 | `<RXC>` senza trasmissione: il testo prima restava | pulisce quello che c'era prima |
| 8 | `<TUNE>`, `<BOOKMARK>`, `<MACROS:...>` erano letti ma la pagina non li gestiva (finivano nel ramo generico) | avviso «non disponibile» esplicito; nel catalogo «non ancora» |
| 9 | `<CATCMD:...>` funzionava ma il catalogo lo dava «non ancora» | catalogo corretto |
| 10 | `<JUMPTOTXFQ>`, `<ALIGN:Hz>`, `<ACQDELAY:s>`, `<ASAUTOCQ>`, `<AUTOCQRESUME>` non eseguiti | eseguiti |
| 11 | `<AUTOCQ>` seguito da testo ignorava il testo | il testo diventa la chiamata ripetuta (comandi riempiti a ogni giro) |
| 12 | Le macro evento OnStart, OnEnd, OnTX, OnCR, OnClickAny si potevano salvare ma non partivano | partono |
| 13 | Comandi capiti dal programma ma assenti dal catalogo (`<ANYQSOBEFORE...>`, `<TEXTFILE:>`, `<IMD>`, `<[> <]>`, `<STARTSCAN>`/`<STOPSCAN>`, forme `OFF`/alternate) | aggiunti come voci o come altri nomi |
| 14 | In olandese le spiegazioni dei comandi erano solo in inglese | dizionario olandese per impronta del testo inglese |

## Tabella dei comandi

### Dati, ora, formattazione

| Comando | Esito | Note |
|---|---|---|
| `<MYCALL>` `<MYMAINCALL>` `<MYNAME>` `<MYQTH>` `<MYWWLOC>` | OK | dalla stazione e dai dati personali |
| `<CALL>` `<NAME>` `<QTH>` `<WWLOC>` `<NOTES>` | OK | dal QSO in corso |
| `<RSTS>` `<RSTR>` | OK | 599 predefinito con «Use default RST» |
| `<ANYQSOBEFOREDATE>` `<ANYQSOBEFOREBAND>` `<ANYQSOBEFOREMODE>` | OK | letti dal log prima di eseguire; mancavano nel catalogo (CORRETTO il catalogo) |
| `<IMD>` | PARZIALE | non scrive niente (la misura si vede nella barra) |
| `<VER>` | OK | |
| `<DATE>` `<TIME>` `<LOCALTIME>` `<STARTTIME>` `<CTIME>` | OK | |
| `<GA>` `<GAL>` | PARZIALE | secondo l'ora UTC, non l'ora del corrispondente |
| `<CR>` `<LF>` `<CRLF>` `<TAB>` `<[>` `<]>` | OK | |

### Frequenza e banda

| Comando | Esito | Note |
|---|---|---|
| `<KHZ>` `<MHZ>` (`:n` decimali) `<BAND>` | OK | vuoto senza radio |
| `<AUDIOFQ:Hz>` (`+`/`-` relativo) | OK | |
| `<SEEKLEFT>` `<SEEKRIGHT>` | OK | |
| `<FQ:kHz>` | OK | la radio si sposta perché il segnale cada sul tono di ricezione |
| `<SPLIT:1>` `<SPLIT:0>` | OK | = Lock della trasmissione |
| `<JUMPTOTXFQ>` | CORRETTO | prima non eseguito |
| `<ALIGN:Hz>` | CORRETTO | prima non eseguito |
| `<STARTSCAN>` `<STOPSCAN>` | OK | mancava nel catalogo |
| `<BOOKMARK>` | NON DISPONIBILE | Aurora non ha segnalibri sulla cascata |

### Trasmissione e ricezione

| Comando | Esito | Note |
|---|---|---|
| `<TX>` | CORRETTO | testo prima di `<RX>`; da solo la casella; si accoda se si trasmette già; mai trasmissione vuota |
| `<TXE>` | OK | attesa della caduta del segnale, al massimo 30 s, Esc annulla |
| `<TXTOGGLE>` | CORRETTO | in trasmissione chiude |
| `<RX>` | CORRETTO | segno nel testo; il testo dopo resta nella finestra |
| `<RXANDCLEAR>` `<RXC>` | CORRETTO | pulizia di casella e storico a fine trasmissione (commit precedente per lo storico, qui la posizione) |
| `<RXE>` | OK | ricezione a fine testo, il testo dopo parte |
| `<TUNE>` | NON DISPONIBILE | il motore dei modi testuali non ha un tono di accordo |

### Finestre e avvisi

| Comando | Esito | Note |
|---|---|---|
| `<CLEARRXWINDOW>` | OK | |
| `<CLEARTXWINDOW>` | CORRETTO | secondo la posizione |
| `<CLEARWINDOW>` | CORRETTO | ricezione e trasmissione, con la posizione |
| `<CURSOR:TX>` | OK | |
| `<BEEP:ON>` | OK | `<BEEP:OFF>` non serve (nessun avviso continuo) |
| `<ALARM:testo>` | OK | avviso a schermo |
| `<HIDETEXT>` `<SHOWTEXT>` | NON DISPONIBILE | |

### QSO, log e contest

| Comando | Esito | Note |
|---|---|---|
| `<NRS>` | OK | vuoto = numero progressivo a tre cifre (ultimo + 1); cifre in chiaro (niente numeri tagliati) |
| `<NRR>` `<QSONR>` | OK | |
| `<CLEARQSO>` `<FIXSTARTTIME>` | OK | |
| `<SAVEQSO>` | CORRETTO | nell'ordine scritto |
| `<IGNOREBEFORE:...>` `<PERIOD:n>` | OK | segni del log di BIGScan fuori gara |

### Modo e impostazioni

| Comando | Esito | Note |
|---|---|---|
| `<MODE>` (testo) `<MODE:x>` | OK | PSK31, PSK63, RTTY, CW; un modo che non c'è viene detto |
| `<BAUDRATE:n>` | PARZIALE | 31.25/62.5 cambiano il modo PSK; in RTTY 30-300 baud |
| `<SHIFT:Hz>` | OK | |
| `<AFCON>` `<AFCOFF>` `<AFC>`, `<SQUELCHON>` `<SQUELCHOFF>` `<SQUELCH>`, `<SNAPON>` `<SNAPOFF>` `<SNAP>` | OK | |
| `<LOCKTX>` `<UNLOCKTX>` `<LOCKTXTOGGLE>` | OK | |
| `<MODESETTINGS>` `<THRESHOLD>` | OK | aprono le impostazioni |
| `<MEMW:n>` `<MEMR:n>` | OK | 1-20 |

### Auto CQ

| Comando | Esito | Note |
|---|---|---|
| `<AUTOCQ>` `<ACQ>` | CORRETTO | da solo accende/spegne (riprende se in pausa); con testo dopo, il testo è la chiamata |
| `<ASAUTOCQ>` | CORRETTO | accende; il testo dopo è la chiamata |
| `<AUTOCQRESUME>` | CORRETTO | riprende (o accende) |
| `<ACQDELAY:s>` | CORRETTO | pausa fra le chiamate, 0-600 s (fino a riavvio della pagina) |

### Rotore, CAT, file

| Comando | Esito | Note |
|---|---|---|
| `<ROTOR:gradi>` `<ROTOR:SP>` `<ROTOR:LP>` | OK | SP/LP dai due locatori |
| `<CATCMD:...>` | CORRETTO | funzionava, il catalogo lo dava «non ancora» |
| `<TEXTFILE:nome>` | OK | mancava nel catalogo |
| `<EDITMACRO:nome>` | OK | |
| `<MACROS:file>` `<MODEMACROS:file>` | NON DISPONIBILE | Aurora non ha file di macro: le macro di ogni modo stanno nelle Impostazioni |

### Modi

| Comando | Esito | Note |
|---|---|---|
| `<AR>` `<BT>` `<KN>` `<SK>` | OK | segno unico in CW, lettere negli altri modi |
| `<WPM:n>` | PARZIALE | porta anche in CW se si è in un altro modo |
| `<LANGUAGE:xx>` `<WK:...>` | NON DISPONIBILE | |
| `<LETTERS>` `<NUMBERS>` | NON DISPONIBILE | il cambio lettere/cifre RTTY lo fa il motore da solo |
| `<LOADPICTURE>` `<FROMCLIP>` `<FONT:>` `<LIMITSCANLINES:>` | NON DISPONIBILE | SSTV ha la sua pagina |
| `<SET FILTER:>` `<SET FONT:>` `<SET SUBMODE:>` | NON DISPONIBILE | Aurora non ha Hellschreiber |

## Macro evento

| Nome | Esito | Quando parte |
|---|---|---|
| Gray+ | OK | tasto + del tastierino |
| AutoCqText | OK | la chiamata dell'Auto CQ (se manca F2) |
| OnStart | CORRETTO | apertura della pagina |
| OnEnd | CORRETTO | chiusura della pagina |
| OnTX | CORRETTO | inizio di una trasmissione |
| OnCR | CORRETTO | Invio nella casella di trasmissione |
| OnClickAny | CORRETTO | una parola della ricezione va in un campo del log |
| OnAlarm, OnSP0, OnSP1, Hello | NON DISPONIBILE | nessun evento corrispondente in Aurora |

## Prove

- `app/src/lib/macroPlan.test.ts`: ordine e posizione (`<RX>` in mezzo, testo dopo `<RX>`, `<CLEARTXWINDOW>` prima e dopo il testo,
  trasmissione in corso, `<TXTOGGLE>`, `<SAVEQSO><CLEARQSO>`), `splitAutoCq`, l'effetto di ogni comando di programma, la coerenza del
  catalogo (categoria ed esempio per ogni voce, esempi senza comandi sconosciuti, ogni comando del lettore presente nel catalogo).
- `app/src/components/digi/MacroGuide.test.tsx`: la Guida mostra tutti i comandi e le macro evento, il filtro, Esc.
- Le prove con la radio le fa il proprietario.
