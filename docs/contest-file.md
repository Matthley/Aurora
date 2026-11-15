# Il file `.contest` (contest dei modi testuali)

I contest della pagina dei modi testuali (PSK, RTTY, CW) stanno nella cartella `contest` dei dati di Aurora
(`%APPDATA%\Aurora\contest` su Windows), un file per contest:

```
contest/
  <nome>.contest      un contest
  macros/<nome>.mc    le macro del contest
  dom/<nome>.dom      gli elenchi dei domini (province, stati, zone)
  attivo.txt          il contest attivo (solo il nome del file, vuoto = «Nessun contest»)
  .predefiniti        segno che i contest distribuiti con Aurora sono gia' stati copiati
```

Al primo avvio Aurora copia i contest distribuiti con l'installatore (`desktop/src-tauri/contest`, risorsa `contest`) senza
sovrascrivere niente; poi i file sono dell'utente: quelli cancellati non tornano. Un solo contest e' attivo alla volta.
Il codice sta in `core/crates/contest/src/elenco.rs`, le rotte in `core/crates/remote-api/src/contest_testo_api.rs`
(`/api/v1/contest-testo`), l'interfaccia in `app/src/components/digi/ContestListDialog.tsx` e `app/src/lib/textContest.ts`.

## Formato

File di testo INI (UTF-8; si leggono anche i vecchi file Latin-1), sezione `[Contest]`, una riga `chiave=valore`. Le chiavi
sconosciute si ignorano; quelle mancanti prendono il valore di base. I booleani sono `0`/`1`.

| Chiave | Significato | Uso in Aurora |
|---|---|---|
| `m_sName` | nome del contest | elenco, barra di stato |
| `m_sNotes` | note (modi, bande) | colonna Note |
| `m_dtStart`, `m_dtEnd` | inizio e fine UTC, `aaaammgghhmmss` (vuoto = nessun limite) | finestra dei doppi; le date valgono ogni anno (si portano nell'anno corrente) |
| `m_iWorkTime` | ore di lavoro ammesse | solo promemoria |
| `m_sNrs` | numero da mandare | con l'incremento: primo numero (001 se vuoto); senza: scambio fisso di `<NRS>` |
| `n_bAutoIncNrs` | incremento automatico del numero | `<QSONR>`/`<NRS>` avanzano a ogni QSO salvato |
| `m_iValidContacts` | tipo di contatti validi | conservato |
| `m_bValidCw`, `m_bValidPhone`, `m_bValidDigital` | categorie valide (nessuna = tutte) | i QSO di categorie non valide non fanno doppio ne' punteggio |
| `m_iPointsMyDxcc`, `m_iPointsMyCont`, `m_iPointsDx` | punti stesso paese / continente / DX | punti per QSO (tutti 0 = un punto) |
| `m_iMultipliers`, `m_iMultipliersOption`, `m_bBandMult` | moltiplicatori | tipo 0-5 (vedi sotto), si sommano; per banda |
| `m_bBand160Mult` … `m_bBand10Mult`, `m_iBand160Mult` … `m_iBand10Mult` | moltiplicatore e peso per banda (160, 80, 40, 20, 15, 10) | peso dei moltiplicatori della banda |
| `m_sMacrosFilename` | file di macro, relativo: `macros/<nome>.mc` | macro del contest sui tasti |
| `m_bMergeMacros` | unisci alle macro del modo | acceso: restano le macro del modo sui tasti liberi |
| `m_bAutoGrabNumber` | prendi il numero ricevuto | il numero dopo il nominativo va nel numero ricevuto, se vuoto |
| `m_iGrabNumberType` | tipo di numero | conservato |
| `m_bAutoGrabTime` | prendi l'ora | conservato |
| `m_bAutoGrabState` | prendi il dominio | la parola che e' nell'elenco dei domini va nel numero ricevuto, se vuoto |
| `m_sStateListFile` | elenco dei domini, relativo: `dom/<nome>.dom` | cattura del dominio |
| `m_iGrabDomainTo` | campo del dominio | conservato (Aurora usa sempre il numero ricevuto) |
| `m_bSuggestCqZone`, `m_bSuggestItuZone`, `m_bSuggestDomain`, `m_bSuggestDxccPfx` | suggerimenti | conservati |
| `m_bDupeNoBand` | doppio su qualsiasi banda | doppi di BIGScan |
| `m_bDupeNoMode` | doppio in qualsiasi modo | doppi di BIGScan |
| `m_iLogBarDispMode` | vista della barra del log | conservato |

I percorsi devono essere relativi alla cartella `contest` (niente unita', niente `..`): il salvataggio li rifiuta.
Nel file non vanno nominativi personali: il proprio nominativo viene dalla Stazione (`<MYCALL>`).

## Punteggio e Cabrillo

Il codice sta in `core/crates/contest/src/punti_testo.rs`, le rotte `GET /api/v1/contest-testo/punteggio` e
`GET /api/v1/contest-testo/cabrillo`, la finestra delle statistiche in `app/src/components/digi/ContestStatsDialog.tsx`.

- QSO: quelli del log di Aurora salvati dalla pagina dei modi testuali (gli scambi stanno in coda al commento,
  `NRS 001 NRR 015`), dentro la finestra di date del contest (portata nell'anno corrente) e di una categoria valida.
- Doppio: stesso nominativo, stessa banda (salvo `m_bDupeNoBand`) e stesso modo (salvo `m_bDupeNoMode`): zero punti, nessun
  moltiplicatore; resta nel Cabrillo.
- Punti: `m_iPointsMyDxcc` stesso paese DXCC della stazione, `m_iPointsMyCont` stesso continente, `m_iPointsDx` gli altri
  (paese sconosciuto = DX); tutti a 0 = un punto per QSO.
- Moltiplicatori (`m_iMultipliers` e `m_iMultipliersOption`, i due tipi si sommano): 0 nessuno, 1 paese DXCC, 2 prefisso WPX,
  3 zona CQ, 4 zona ITU, 5 dominio (prima parola dello scambio ricevuto trovata nell'elenco `.dom`; senza elenco l'ultima parola
  non numerica). Le zone sono quelle di cty.dat, non quelle scritte nello scambio.
- `m_bBandMult`: i moltiplicatori si contano su ogni banda; una banda con `m_bBandNNMult=1` e peso > 0 conta ogni suo
  moltiplicatore per il peso, le altre per 1.
- Punteggio = punti x moltiplicatori (senza tipo di moltiplicatore: i punti).
- Cabrillo 3.0: `CONTEST:` (nome in maiuscolo con i trattini), `CALLSIGN`, `CATEGORY-OPERATOR: SINGLE-OP`, `CATEGORY-BAND: ALL`,
  `CATEGORY-MODE` dai QSO (CW, RTTY, SSB, DIGI o MIXED), `CATEGORY-TRANSMITTER: ONE`, `CLAIMED-SCORE`, `CREATED-BY`; righe
  `QSO: freq(kHz) modo(CW/RY/PH/DG) aaaa-mm-gg hhmm mio-call rst+scambio-inviato call rst+scambio-ricevuto`. Potenza, assistenza
  e le altre categorie si aggiungono a mano prima dell'invio.

Queste chiavi valgono solo per i file senza sezione `[Score]` (vedi sotto), che ha la precedenza.

## Le regole del punteggio: sezione `[Score]`

Dopo `[Contest]` un file puo' avere la sezione `[Score]` con le regole vere del contest, fatte di pezzi riusabili (codice in
`core/crates/contest/src/regole_punti.rs`, regolamenti riassunti in `docs/contest-regole.md`). Una chiave per riga:

| Chiave | Significato | Esempio |
|---|---|---|
| `points` | regole dei punti `condizione=punti` separate da `;`: vince la prima vera; nessuna vera = 0. `=cqtab` usa la tabella delle zone | `same_dxcc=1; same_cont=2; other_cont=3` |
| `points_band` | fattore dei punti per banda | `80:2,40:2` |
| `bands` | bande ammesse (le altre: QSO non ammesso) | `80,40,20,15,10` |
| `valid` | condizione dei QSO ammessi (per i contest validi solo con un paese) | `dxcc:F` |
| `mult` | moltiplicatori `tipo/opzione/...` separati da `;` | `cq/band; dxcc/band; exch/band/if:dxcc:K,VE` |
| `mult_band` | peso dei moltiplicatori per banda (solo quelli con `band`) | `80:4,40:3,20:2,15:2,10:2` |
| `bonus` | punti in piu' `condizione=punti` (si sommano tutti quelli veri) | `call:RAEM=300; dxcc:EA=5` |
| `cq1` ... `cq40` | tabella dei punti zona CQ -> zona CQ: riga della propria zona, 40 valori separati da virgola | `cq15=10,10,...` |
| `special` | regole scritte a mano | `raem` |

Condizioni (pezzi uniti da `&`, tutti veri): `any`, `same_dxcc` (stesso paese della stazione), `same_cont` (stesso continente),
`other_cont` (altro continente o sconosciuto), `dxcc:K,VE`, `not_dxcc:K,VE`, `cont:EU,AS`, `my_dxcc:...`, `my_cont:...`, `call:RAEM`
(nominativo o una sua parte fra le barre), `band:80,40`, `mode:RTTY,BPSK63`. I paesi sono i prefissi principali del database DXCC di
Aurora (cty.dat): `K` Stati Uniti, `VE` Canada, `UA` Russia europea, `UA9` asiatica, `UA2` Kaliningrad, `UR` Ucraina...

Tipi di moltiplicatore: `dxcc` paese, `cq` e `itu` zone (dal database DXCC), `wpx` prefisso WPX, `cont` continente, `area:K,VE,JA`
area numerica dei paesi elencati (`K1`, `VE3`: la cifra dopo la barra `K5DJ/1`, altrimenti quella del prefisso), `exch` lo scambio
ricevuto (la parola trovata nell'elenco `.dom` del contest, oppure l'ultima parola non numerica: stato, provincia, oblast).
Opzioni: `band` per banda (con il peso di `mult_band`), `mode` per modo, `once` una volta sola (il predefinito), `product` moltiplica
a parte, `if:<condizione>` solo per i QSO che la rispettano, `except:<paesi>` non per questi paesi.

Punteggio = (punti + QTC) x (somma dei moltiplicatori pesati senza `product`) x (somma di quelli `product`) x fattore finale. Un QSO
non ammesso (`bands`, `valid`) vale zero punti e nessun moltiplicatore e non fa doppio. Regole scritte a mano: `special=raem` aggiunge
un punto per grado di differenza di latitudine e longitudine fra le coordinate mandate e ricevute (`57N85E`), 100 per una stazione
polare (latitudine 66 o piu') e moltiplica il totale per 1,1 se la stazione sta dentro i circoli polari.

Esempio (CQ WW RTTY):

```
[Score]
bands=80,40,20,15,10
points=same_dxcc=1; same_cont=2; other_cont=3
mult=cq/band; dxcc/band; exch/band/if:dxcc:K,VE
```

Il salvataggio rifiuta una sezione con chiavi, condizioni o tipi sconosciuti. La finestra delle statistiche mostra punteggio, ritmo
(QSO all'ora sugli ultimi 10 minuti, QSO nell'ultima ora, media), QSO non ammessi, moltiplicatori nuovi e la griglia banda x
moltiplicatori con quelli che mancano (zone, continenti, aree e domini, che hanno un elenco chiuso). `GET /api/v1/contest-testo/ammesso?call=&band=&mode=&exch=`
dice di un nominativo se e' ammesso (motivo `band` o `valid`), se e' doppio, quanti punti vale e quali moltiplicatori nuovi porta.

I file distribuiti con la sezione `[Score]` sono quelli di `docs/contest-regole.md`; gli altri hanno ancora le vecchie chiavi a 0
(un punto per QSO, nessun moltiplicatore).

## File di macro `.mc`

Sezione `[Macros]`: `nItems=n`, poi per ogni macro `NameI` (tasto: `F1`…`F12`, `Ctrl-F1`…, oppure `AutoCqText`, `OnStart`…),
`LabelI` (scritta del tasto), `TextI` (testo con i comandi `<...>` del linguaggio delle macro), `BkColorI` (colore del tasto,
colore Windows `0x00BBGGRR` in decimale).

## Elenco dei domini `.dom`

Riga di titolo facoltativa, `COUNTRY = <prefisso>` per il paese a cui vale, sezioni `PREFISSO:` (le righe che seguono valgono
per i nominativi che iniziano cosi'), righe `ricevuto = dominio` (`Milano = MI`). La ricerca guarda prima le sezioni che
corrispondono al nominativo, poi tutto l'elenco.

## Contest predefiniti e convertitore

I contest in `desktop/src-tauri/contest` (135 contest, 132 file di macro, 10 elenchi di domini) vengono da file di contest di
utenti nello stesso formato INI, convertiti con

```
cargo run -p contest --example converti_contest -- <origine> desktop/src-tauri/contest
```

(`<origine>` con le cartelle `Contest-Settings`, `Contest-Macros`, `Contest-DOM`). Il convertitore:

- toglie il nominativo personale (`m_sMyCall`) e lo sostituisce con `<MYCALL>` anche nel testo delle macro;
- toglie i moduli di statistiche esterni (`m_sStatsDllName`, `m_sStatsSetName`, `m_iStatsSetNumber`): nessuno corrisponde alle
  regole dei contest FT8/FT4 di Aurora, quindi questi contest non hanno punteggio;
- rende relativi i percorsi delle macro e dei domini; se il file di macro indicato manca, prova con il nome del contest;
- toglie dalle macro i programmi esterni (`<EXEC:...>`) e i percorsi dei file audio (`<WAVE:...>` resta col solo nome del file);
- ripara le date non valide (tolte) e le fini precedenti all'inizio (prendono l'anno dell'inizio).

La prova `converte_tutti_i_file_di_origine` (con `AURORA_CST_DIR=<origine>`) converte tutti i file e controlla che non restino
nominativi, percorsi assoluti o moduli esterni; `predefiniti_del_repository_puliti` controlla i file distribuiti.
