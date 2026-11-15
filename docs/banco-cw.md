# Banco del ricevitore CW a caratteri

Prova `core/crates/mode-digi/tests/banco_cw.rs`: misura il ricevitore CW di Aurora carattere per carattere, prima e dopo ogni modifica,
sul canale singolo (`CwDemodulator` al tono giusto, impostazioni di base del servizio) e su BigScan CW (`CwBandDecoder` su tutta la banda:
si raccoglie il testo dei canali entro 25 Hz dal tono).

```
CARGO_BUILD_JOBS=2 nice cargo test --release -j 2 -p mode-digi --test banco_cw -- --ignored --nocapture --test-threads=1
```

Circa 75 secondi con 3 semi; le tabelle qui sotto sono con `CW_BANCO_SEMI=8` (circa 6 minuti). Variabili: `CW_BANCO_SEMI` (semi per scenario, di base 3), `CW_RIGHE` (scenari il cui nome contiene uno dei pezzi,
separati da virgole), `CW_BIGSCAN=0` (solo canale singolo), `CW_DEBUG=1` (stampa mandato e ricevuto). Senza `--ignored` girano tre prove
veloci (allineamento, testo pulito, poco rumore).

## Segnale

- Testo da contest generato con seme fisso: `CQ TEST IU3ABC IU3ABC`, `K1XYZ 5NN 014`, `TU ... TEST`, `R 5NN 123 TU`, `CQ ... IO91WM K`, con
  nominativi inventati (prefissi a caso, una cifra, 1-3 lettere). Circa 40 s di Morse per caso.
- Tono 700 Hz, fronti a coseno di 4 ms, rumore bianco gaussiano; S/N riferito a 500 Hz (potenza del tono acceso contro il rumore in
  500 Hz). Per confronto: S/N in 3 kHz = S/N in 500 Hz - 7,8 dB.
- Manipolazione da keyer: irregolarita' del 3% su ogni elemento.

## Scenari

| nome | cosa |
|---|---|
| `18/25/32/40wpm +10` | velocita' diverse, segnale buono |
| `25wpm +3 / 0 / -3 / -6`, `40wpm 0` | S/N da buono a molto basso |
| `salto 18-35` | la velocita' passa da 18 a 35 WPM a meta' testo |
| `manuale +10 / +3` | tasto a mano: irregolarita' 15-25% su ogni elemento, pause fra lettere e parole allungate fino al 60% |
| `qsb lento`, `qsb lento 0` | evanescenza di Rayleigh con banda Doppler 0,1 Hz (S/N medio +6 e 0 dB) |
| `qsb veloce` | evanescenza di Rayleigh con banda Doppler 2 Hz (sfarfallio) |
| `crepitii` | scariche di 1-5 ms, da 5 a 30 volte il rumore, circa 4 al secondo |
| `vicina +20` | un'altra stazione 20 dB piu' forte a +150 Hz, testo e velocita' suoi |
| `silenzi`, `silenzi 0`, `silenzi 32` | trasmissioni di 3-6 parole separate da 4-8 s di silenzio (lettere spurie all'avvio e alla fine) |
| `rumore 60s` | un minuto di solo rumore |

## Misure

- **giusti, sbagliati, mancanti, inseriti**: allineamento di Levenshtein fra il testo mandato e quello ricevuto (maiuscole, spazi singoli,
  segni di procedura contati come un carattere). **CER** = (sbagliati + mancanti + inseriti) / caratteri mandati; **ns** = lo stesso
  senza gli spazi (separa le lettere sbagliate dalle parole spezzate o unite, tipiche del tasto a mano).
- **call**: nominativi mandati ritrovati esatti come parola, nell'ordine; **falsi**: parole ricevute che hanno la forma di un nominativo (i locatori no)
  ma non sono fra quelli mandati.
- **falsi/min** sul solo rumore: caratteri non spazio stampati al minuto (BigScan: da tutti i canali).
- **lat**: mediana e 95° percentile del tempo fra la fine del carattere trasmesso e la sua stampa (l'audio entra a blocchi di 100 ms).
- **cpu**: millisecondi di calcolo per secondo d'audio (un thread; per BigScan tutto il decodificatore della banda).

## Risultati

8 semi per scenario, stesso banco. «Prima» e' il codice del commit del banco, «dopo» quello con i quattro cambi tenuti (sotto). CER in %:

| scenario | canale prima | canale dopo | BigScan prima | BigScan dopo |
|---|---|---|---|---|
| 18/25/32/40 WPM +10, 25 WPM +3 | 0,0 | 0,0 | 0,0 | 0,0 |
| 25 WPM 0 | 3,8 | 2,1 | 3,6 | 1,8 |
| 25 WPM -3 | 35,4 | 23,8 | 32,9 | 24,3 |
| 25 WPM -6 | 87,4 | 83,8 | 90,7 | 89,9 |
| 40 WPM 0 | 14,5 | 11,6 | 15,0 | 11,9 |
| salto 18-35 | 1,1 | 1,1 | 0,8 | 0,9 |
| manuale +10 | 17,0 | 8,6 | 17,1 | 8,8 |
| manuale +3 | 18,2 | 10,5 | 18,4 | 10,9 |
| qsb lento | 6,3 | 5,3 | 5,6 | 5,3 |
| qsb lento 0 | 28,0 | 24,0 | 27,0 | 25,4 |
| qsb veloce | 60,2 | 55,4 | 59,9 | 56,2 |
| crepitii | 0,0 | 0,0 | 0,0 | 0,0 |
| vicina +20 | 1,3 | 1,3 | 1,4 | 1,0 |
| silenzi / silenzi 32 | 0,0 | 0,0 | 0,0 | 0,0 |
| silenzi 0 | 2,5 | 2,1 | 2,1 | 1,6 |
| **CER medio per scenario** | **13,79** | **11,47** | **13,72** | **11,90** |

| totali | canale prima | canale dopo | BigScan prima | BigScan dopo |
|---|---|---|---|---|
| caratteri giusti | 16320 | 16571 | 16340 | 16499 |
| sbagliati / mancanti / inseriti | 1104 / 858 / 474 | 952 / 759 / 323 | 1035 / 907 / 486 | 899 / 884 / 328 |
| nominativi esatti (su 938) | 693 | 716 | 687 | 715 |
| nominativi falsi | 171 | 157 | 169 | 161 |
| falsi sul rumore (40 min) | 0 | 1 carattere | 0 | 0 |
| latenza mediana / 95° | 1,23 / 2,11 s | 1,25 / 2,13 s | 1,24 / 2,11 s | 1,25 / 2,13 s |
| calcolo | 10,5 ms/s | 10,7 ms/s | 5,4 ms/s | 5,4 ms/s |

Il carattere falso in 40 minuti di rumore e' uno solo e cambia posto a ogni piccola variazione dei parametri (togliendo uno qualunque
dei cambi sparisce, rimettendolo torna): e' il limite del cancello sullo spettro, non una tendenza.

### Cambi tenuti (tutti in `cw_bayes.rs`)

Misure su 8 semi, canale singolo (CER medio per scenario e caratteri giusti su tutte le righe):

| cambio | CER medio | giusti | nominativi esatti / falsi |
|---|---|---|---|
| partenza | 13,6 * | 16320 | 693 / 171 |
| confine fra le classi di pausa dalle densita' pesate con le probabilita' a priori, non dalla media geometrica | 12,4 | 16372 | 705 / 273 ** |
| larghezze di partenza del pugno 0,8 volte | 11,8 | 16492 | 702 / 277 ** |
| probabilita' a priori delle pause 67/27/6 (prima 60/30/10) | 11,4 | 16510 | 710 / 270 ** |
| pesi della miscela delle ampiezze 0,1/0,1/0,65/0,15 (prima 0,1/0,2/0,55/0,15) | 11,1 | 16571 | 716 / 267 ** |

\* CER totale sui caratteri, non medio per scenario. ** con il conteggio vecchio, che contava il locatore IO91WM come nominativo falso.

### Provati e lasciati

| prova | esito |
|---|---|
| cancello sullo spettro piu' basso (-18 e -30 dB invece di -16) | a -6 dB nessun guadagno, sul rumore 2,8 caratteri falsi al minuto con -30 |
| fattore d'ampiezza 3,0 in piu' nella miscela | peggio (25 WPM 0 e 40 WPM 0) |
| peso dell'ampiezza «che spiega meglio il tono» 0, 0,012, 0,06, 0,08 | peggio o uguale |
| apprendimento del pugno piu' svelto (dopo 8 elementi, passo 0,08) | nominativi esatti +10, ma il canale perde 23 caratteri giusti (a -6 dB) e guadagna 8 nominativi falsi |
| larghezze del pugno 1,2 volte | peggio (CER 12,3 -> 14,3) |
| larghezze del pugno 0,7 volte | il tasto a mano peggiora e compaiono caratteri falsi sul rumore |
| larghezze del pugno 0,93 volte (dopo gli altri cambi) | guadagno minimo, entro il rumore della misura |
| attesa di 3 caratteri prima di stampare | stesso CER, latenza piu' alta |
| attesa di 1 carattere | latenza mediana 1,25 -> 0,97 s ma qualche carattere sbagliato in piu' (CER +0,1) |
| attesa massima 1,0 s invece di 1,5 | uguale |
| peso del modello del testo 0,3 e 0,8 | peggio |
| BEAM_DROP 20 | uguale |
| ipotesi 200 invece di 320 | un po' peggio |
| rapporto segnale/rumore massimo creduto 10 invece di 5 | peggio |
| probabilita' delle pause lunghe 0,15 e 0,08 | meglio sulle righe difficili, peggio sui silenzi |
| velocita' che cambia meno fra i caratteri | peggio |
| ampiezza libera pezzo per pezzo (evanescenza veloce) mescolata al 15% | peggio, anche sull'evanescenza veloce |
| BigScan: soglia di apertura dei canali 8 dB invece di 10 | peggio |

## Seconda taratura: pezzi sommati in fase di due unita'

Nel giudizio di un tono (`mark_llr`) i campioni si sommano in fase a pezzi; i pezzi erano lunghi un'unita' della velocita' migliore,
ora due (`PIECE_UNITS`). Con la fase del tono gia' corretta, sommare in fase su tratti piu' lunghi tira fuori i segnali deboli. 8 semi,
«prima» = la tabella qui sopra (colonne «dopo»), CER in %:

| scenario | canale prima | canale dopo | BigScan prima | BigScan dopo |
|---|---|---|---|---|
| 25 WPM 0 | 2,1 | 1,5 | 1,8 | 1,6 |
| 25 WPM -3 | 23,8 | 20,5 | 24,3 | 22,2 |
| 25 WPM -6 | 83,8 | 82,4 | 89,9 | 86,1 |
| 40 WPM 0 | 11,6 | 9,3 | 11,9 | 12,1 |
| salto 18-35 | 1,1 | 1,1 | 0,9 | 0,6 |
| manuale +10 | 8,6 | 9,0 | 8,8 | 9,2 |
| manuale +3 | 10,5 | 10,1 | 10,9 | 11,3 |
| qsb lento | 5,3 | 6,0 | 5,3 | 5,6 |
| qsb lento 0 | 24,0 | 21,5 | 25,4 | 22,1 |
| qsb veloce | 55,4 | 52,2 | 56,2 | 51,7 |
| vicina +20 | 1,3 | 1,1 | 1,0 | 1,1 |
| silenzi 0 | 2,1 | 0,9 | 1,6 | 0,9 |
| altre righe | 0,0 | 0,0 / 0,1 | 0,0 | 0,0 / 0,1 |

| totali | canale prima | canale dopo | BigScan prima | BigScan dopo |
|---|---|---|---|---|
| caratteri giusti | 16571 | 16719 | 16499 | 16616 |
| sbagliati / mancanti / inseriti | 952 / 759 / 323 | 923 / 640 / 340 | 899 / 884 / 328 | 840 / 826 / 330 |
| nominativi esatti (su 938) | 716 | 737 | 715 | 733 |
| nominativi falsi | 157 | 136 | 161 | 138 |
| falsi sul rumore (8 min per riga) | 1 carattere | 0 | 0 | 0 |
| latenza mediana / 95° | 1,25 / 2,13 s | 1,25 / 2,12 s | 1,25 / 2,13 s | 1,25 / 2,13 s |
| calcolo | 10,7 ms/s | 10,5 ms/s | 5,4 ms/s | 5,3 ms/s |

### Provati e lasciati (dopo i pezzi di due unita', solo canale)

| prova | esito |
|---|---|
| pezzi di 1,5 e 3 unita' | giusti 16630 e 16685, nominativi 714 e 726: peggio di 2 |
| apprendimento dell'ampiezza pesato con il rapporto S/N del tono (prima dei pezzi lunghi) | giusti 16571 -> 16546 |
| probabilita' a priori 56/44 fra punto e linea quando l'albero ammette entrambi | giusti +15, nominativi +4, ma torna 1 carattere falso sul rumore |
| peso dell'ampiezza «che spiega meglio il tono» 0,06 | giusti +11, nominativi -1, 1 carattere falso sul rumore |
| apprendimento del pugno dopo 14 elementi invece di 20 | giusti 16719 -> 16701 |

## Terzo giro: idee generali sul riconoscimento, provate una alla volta

Partenza = tabelle della seconda taratura (canale: giusti 16719, nominativi 737 / 136 falsi, CER medio 10,78%; BigScan: giusti 16616).
Regola: si tiene solo cio' che non perde caratteri giusti ne' nominativi e non aggiunge falsi. 8 semi, solo canale salvo dove detto.

| prova | giusti | nominativi esatti / falsi | CER medio | esito |
|---|---|---|---|---|
| pausa oltre il confine del carattere tenuta anche come pausa fra gli elementi (coda della durata, ipotesi in piu' nel traliccio) | 16704 | 737 / 133 | 10,86 | lasciata: perde caratteri |
| medie del pugno adattate insieme (filtro di Kalman su linea e tre pause, covarianza a priori con pause correlate, incertezza 0,1) | 16708 | 764 / 140 | 10,46 | lasciata: +27 nominativi (tasto a mano 39 -> 63) ma -11 giusti e +4 falsi |
| stessa, correlazioni dimezzate | 16702 | 761 / 141 | 10,54 | lasciata |
| stessa, incertezza a priori 0,05 | 16714 | 748 / 143 | 10,54 | lasciata: -5 giusti, +7 falsi |
| BigScan: spettro mediato piu' lento (0,2 invece di 0,4) per l'apertura dei canali | 16490 (BigScan) | 732 / 134 | 11,90 (BigScan) | lasciata: i canali si aprono tardi, mancanti 826 -> 1070 |

Sul solo rumore nessuna prova ha stampato caratteri.

## Quarto giro: ogni idea provata in piu' varianti

Stessa regola, 8 semi, solo canale (BigScan dove detto). Partenza del canale: giusti 16719, nominativi 737 / 136 falsi.

**Adattamento correlato del pugno (filtro di Kalman sui logaritmi delle medie)** — tenuto nella variante K3:

| variante | giusti | nominativi esatti / falsi | CER medio |
|---|---|---|---|
| K1: solo linea e pausa breve | 16663 | 735 / 141 | 11,17 |
| K2: tutte, pausa fra i caratteri al massimo 4 unita' | 16709 | 761 / 139 | 10,50 |
| **K3: solo le pause fra i caratteri e fra le parole, correlate 0,6** | **16728** | **759 / 143** | **10,43** |
| K4: come K3, incertezza a priori 0,07 | 16716 | 755 / 143 | 10,56 |
| K5: come K3, correlazione 0,3 | 16716 | 756 / 142 | 10,55 |

K3 su BigScan: giusti 16616 -> 16622, nominativi 733 -> 755 (falsi 138 -> 143); tasto a mano 39 -> 61 nominativi esatti su 78. Nessun
carattere sul rumore, crepitii invariati. I nominativi falsi crescono di 7 su 938 mandati, contro 22 esatti in piu'.

Da qui la partenza e' K3 (giusti 16728, nominativi 759 / 143).

**Rumore dalle due spalle** (in `cw_noise`, come si combinano le due bande laterali quando sono pulite tutte e due) — lasciato:

| variante | giusti | nominativi esatti / falsi | CER medio |
|---|---|---|---|
| media geometrica dei due lati | 16735 | 758 / 140 | 10,34 |
| meta' aritmetica e meta' geometrica | 16751 | 758 / 144 | 10,36 |
| media geometrica +10% | 16673 | 754 / 138 | 10,72 |

Piu' caratteri giusti (fino a +23), ma sempre un nominativo esatto in meno.

**Probabilita' dell'inviluppo con due tanh mescolata alla verosimiglianza coerente** (logaritmo del rapporto sommato sui campioni del
tono, quasi zero a cancello chiuso) — lasciata:

| peso / fondo | giusti | nominativi esatti / falsi | falsi sul rumore |
|---|---|---|---|
| 0,05 / 2 | 16642 | 739 / 134 | 1 in 8 min |
| 0,01 / 3 | 16758 | 751 / 135 | 0 |
| 0,003 / 3 | 16775 | 753 / 142 | 1 in 8 min |

Con il peso piccolo i caratteri giusti salgono (+47, CER medio 10,43 -> 10,35) ma si perdono nominativi e torna un carattere sul rumore.

**Media lunga (0,75 s) della potenza sul rumore con isteresi, come cancello di presenza in piu'** — lasciata:

| soglie on / off | giusti | nominativi esatti / falsi | crepitii | rumore |
|---|---|---|---|---|
| 1,3 / 1,15 | 16623 | 750 / 162 | 1 sbagliato | 0 |
| 1,15 / 1,05 | 16655 | 754 / 156 | 0 | 0 |

Sul rumore e sui crepitii non c'era gia' niente da togliere; il cancello in piu' chiude anche sui segnali deboli.

**Confini fra parole scelti dopo la decodifica** (lessico del traffico e forma del nominativo; la parola si tiene una indietro, si unisce o
si spezza, le lettere non cambiano) — lasciato:

| variante | giusti | inseriti | nominativi esatti / falsi | latenza mediana |
|---|---|---|---|---|
| unire e spezzare | 16724 | 264 (prima 290) | 759 / 153 | 2,48 s (prima 1,25) |
| solo spezzare | 16728 | 290 | 759 / 143 | 2,48 s |
| unire solo un pezzo di 1-3 caratteri in un nominativo | 16724 | 278 | 759 / 153 | 2,48 s |

Il traliccio non incolla mai due parole (spezzare non scatta). Unire toglie spazi inventati ma crea nominativi falsi, e la parola tenuta
raddoppia la latenza.

## Quinto giro: un secondo ricevitore completo, a catena (`cw_chain.rs`)

Invece di provare le idee una per volta dentro `cw_bayes.rs`, si e' costruito un ricevitore intero a se' (`mode_digi::cw_chain`:
`CwChain` per il canale, `CwChainBand` per BigScan), misurabile sullo stesso banco con `CW_RX=catena`; `CW_PARAM=nome=valore,...`
cambia i suoi parametri (`ChainParams::set`). Stadi: spettro a 93,75 passi al secondo (1024 punti, finestra a quattro coseni per un sinc
da 25 Hz) e confronto di forma su ±10 righe; inviluppo (mediana di 3, media lunga 71 con cancello a isteresi, massimo 17 e media 9, fondo
col minimo 111); probabilita' di tasto abbassato; sequenza a durate esplicite (20 eta' del tono, 30 del silenzio, code, risalita a blocchi
di 25 su 50 passi); cinque classi lognormali con avvio sulle ipotesi dei primi 14 elementi e adattamento di Kalman a 5 dimensioni; Morse
con divisione dei codici sconosciuti; segmentazione lessicale delle parole e conteggio dei nominativi ripetuti; per BigScan accumulatore,
contrasto a ±4 righe, massimi stretti con maschera e ciclo di vita dei canali.

Cambi necessari rispetto alla descrizione di partenza, trovati sul banco: densita' confrontate in logaritmo e classi in ordine di durata
(lontano dalle medie le densita' si annullavano e una pausa di un passo diventava fine parola); il fondo dell'inviluppo scende solo sui picchi
del tono (col minimo sempre attivo crollava nelle pause lunghe); avvio con unita' separate per toni e pause e quota attesa di ogni classe;
avvio rifatto quando un carattere supera 7 elementi e controllo ogni 14 elementi (cambio di velocita', tasto a mano); ibrido: al posto delle
due tanh la verosimiglianza di Rice contro Rayleigh della potenza della riga (S/N dal minimo fra media lunga e inviluppo corto, spalle
lontane 6-8 righe usate solo per i disturbi larghi e solo se vicine al livello del centro). Le due tanh restano come `emit_mode=0`.

8 semi, CER in %:

| scenario | attuale canale | catena canale | catena con le due tanh | attuale BigScan | catena BigScan |
|---|---|---|---|---|---|
| 18/25/32/40 WPM +10 | 0,0 | 0,1-0,7 | 0,2-28,3 | 0,0 | 0,2-0,5 |
| 25 WPM +3 | 0,0 | 0,6 | 7,2 | 0,0 | 1,3 |
| 25 WPM 0 | 1,5 | 2,5 | 18,6 | 1,6 | 3,9 |
| 25 WPM -3 | 20,1 | 23,7 | 33,2 | 21,7 | 25,9 |
| 25 WPM -6 | 81,7 | 88,3 | 88,9 | 86,8 | 91,3 |
| 40 WPM 0 | 8,7 | 13,7 | 76,3 | 11,6 | 13,7 |
| salto 18-35 | 1,1 | 7,7 | 41,8 | 0,6 | 8,0 |
| manuale +10 / +3 | 6,6 / 7,7 | 14,2 / 14,8 | 15,8 / 31,0 | 7,4 / 8,4 | 14,7 / 14,8 |
| qsb lento / lento 0 | 5,9 / 20,8 | 8,4 / 34,3 | 10,3 / 29,9 | 5,3 / 20,5 | 9,3 / 34,0 |
| qsb veloce | 52,4 | 63,3 | 39,4 | 51,1 | 62,1 |
| crepitii | 0,0 | 3,8 | 2,1 | 0,0 | 3,6 |
| vicina +20 | 1,1 | 10,5 | 17,6 | 1,1 | 74,8 |
| silenzi / 0 / 32 | 0,0 / 0,9 / 0,1 | 1,6 / 5,2 / 2,0 | 2,4 / 16,7 / 12,8 | 0,1 / 0,9 / 0,1 | 4,6 / 9,3 / 5,9 |
| **CER medio** | **10,43** | **14,77** | **23,86** | **10,87** | **18,91** |

| totali | attuale canale | catena canale | catena con le due tanh | attuale BigScan | catena BigScan |
|---|---|---|---|---|---|
| caratteri giusti | 16728 | 16557 | 14223 | 16622 | 16312 |
| sbagliati / mancanti / inseriti | 893 / 661 / 290 | 1547 / 178 / 876 | 3121 / 938 / 535 | 833 / 827 / 274 | 1759 / 211 / 1360 |
| nominativi esatti / falsi | 759 / 143 | 718 / 231 | 509 / 289 | 755 / 143 | 688 / 260 |
| falsi sul rumore (8 min) | 0 | 0 | 0 | 0 | 0 |
| latenza mediana / 95° | 1,25 / 2,13 s | 0,57 / 0,72 s | 0,57 / 0,72 s | 1,26 / 2,13 s | 0,58 / 0,73 s |
| calcolo | 11,2 ms/s | 1,6 ms/s | 1,4 ms/s | 5,7 ms/s | 1,6 ms/s |

Esito: **il ricevitore attuale resta il predefinito**; la catena resta nel codice, spenta (non collegata al servizio). Perde 171 caratteri
giusti sul canale e piu' di 300 su BigScan, con nominativi falsi in piu'. Vince solo sull'evanescenza veloce nella variante con le due tanh
(39,4 contro 52,4), sulla latenza (meno della meta') e sul calcolo (un settimo). Il limite principale e' il fronte: la riga di 11,7 Hz su
85 ms e' incoerente e spalma i fronti (a 40 WPM le pause fra gli elementi durano 2-3 passi), mentre il ricevitore attuale somma in fase.
In BigScan «vicina +20» la catena apre un canale sulla stazione forte che finisce entro 25 Hz dal tono.

Provati e lasciati sulla catena (3 semi): taglio della finestra 35 e 50 Hz (instabile, peggio); varianza minima delle classi 0,02 / 0,05;
peso della verosimiglianza 0,15-1,6 (tenuto 0,8); S/N minimo creduto 1,5 / 10 (tenuto 6); deviazione massima delle classi 0,8 / 1,2;
rumore di processo 0,01; pesi delle pause lunghe 0,3 / 0,05; coda delle pause fra le parole 0,05; segmentazione lessicale con 2-3 lettere
d'attesa (peso 0,3: +3 caratteri giusti e latenza +1,1 s; peso 1: -246 caratteri, il lessico unisce parole).

Da provare: la verosimiglianza coerente di `cw_bayes` come emissione della sequenza a durate esplicite (serve la banda base I/Q dentro la
catena); in BigScan una maschera piu' larga attorno ai canali forti.

## Sesto giro: la catena con il giudizio coerente dei toni (`emit_mode=2`)

Al posto della potenza della riga, la sequenza a durate esplicite giudica ogni tono con la somma in fase dei suoi passi, a pezzi di due
unita', Rice contro Rayleigh con la miscela delle ampiezze come in `cw_bayes`; le pause valgono zero. La banda base e' la riga complessa
di una seconda finestra (passa-basso da 60 Hz) riportata in fase fra uno spettro e l'altro e riferita al rumore: un campione a 93,75 Hz
per canale, senza mixer ne' filtri in piu', valido anche per BigScan. La varianza delle somme tiene conto della sovrapposizione dei passi
(prova: misurata e prevista entro il 15%). Ampiezza e rumore li insegna il percorso deciso (passi accesi e pause lontani dai fronti),
all'avvio il quantile alto della potenza; la rotazione residua della fase si stima sui passi a tasto abbassato. Cambi trovati sul banco:
S/N minimo creduto 2 per passo (6, quello dell'emissione di Rice sulla potenza, uccideva i segnali deboli), S/N massimo 20, tono a
cancello chiuso possibile con penalita' 12 (vietato perdeva caratteri). Lasciati: finestra da 100 e 150 Hz (molto peggio, la stazione
vicina entra), pezzi di un'unita', controllo dell'avvio con margine 4.

BigScan: maschera di 9, 11, 15 e 20 righe attorno ai canali forti e centro che non si allontana piu' di due righe da dove il canale e'
nato: nessun effetto su «vicina +20» (con 15 righe la stazione debole non si apre piu'). Il difetto di «vicina +20» in BigScan con
l'emissione sulla potenza e' un aggancio sbagliato del canale a 703 Hz (velocita' e classi), non un canale in piu'. Le due misure restano
nel codice (`band_mask_strong`, di base spenta).

8 semi:

| totali | attuale canale | catena canale | catena coerente canale | attuale BigScan | catena BigScan | catena coerente BigScan |
|---|---|---|---|---|---|---|
| caratteri giusti | **16728** | 16557 | 15892 | **16622** | 16312 | 15922 |
| CER medio | **10,43** | 14,77 | 16,51 | **10,87** | 18,91 | 15,55 |
| nominativi esatti / falsi | **759 / 143** | 718 / 231 | 630 / 222 | **755 / 143** | 688 / 260 | 637 / 241 |
| falsi sul rumore | 0 | 0 | 0 | 0 | 0 | 0 |
| latenza mediana / 95° | 1,25 / 2,13 s | 0,57 / 0,72 s | 0,56 / 0,71 s | 1,26 / 2,13 s | 0,58 / 0,73 s | 0,57 / 0,72 s |
| calcolo | 11,2 ms/s | 1,5 ms/s | 2,2 ms/s | 5,7 ms/s | 1,5 ms/s | 2,2 ms/s |

| CER % | attuale canale | catena | coerente | attuale BigScan | catena BigScan | coerente BigScan |
|---|---|---|---|---|---|---|
| 25 WPM -3 | 20,1 | 23,7 | 28,6 | 21,7 | 25,9 | 28,9 |
| 40 WPM 0 | 8,7 | 13,7 | 36,7 | 11,6 | 13,7 | 29,3 |
| salto 18-35 | 1,1 | 7,7 | 7,4 | 0,6 | 8,0 | 8,3 |
| manuale +10 / +3 | 6,6 / 7,7 | 14,2 / 14,8 | 23,2 / 13,5 | 7,4 / 8,4 | 14,7 / 14,8 | 14,8 / 13,9 |
| qsb lento / lento 0 | 5,9 / 20,8 | 8,4 / 34,3 | 8,3 / 25,3 | 5,3 / 20,5 | 9,3 / 34,0 | 8,3 / 26,7 |
| qsb veloce | 52,4 | 63,3 | 67,5 | 51,1 | 62,1 | 25,7 |
| vicina +20 | 1,1 | 10,5 | 5,5 | 1,1 | 74,8 | 8,6 |

Esito: **il ricevitore attuale resta il predefinito**; il giudizio coerente resta spento. Toglie il difetto di «vicina +20» in BigScan e
regge meglio il QSB lento a 0 dB, ma perde molto a 40 WPM (la finestra da 60 Hz spalma ancora pause di 2-3 passi) e il risultato cambia
molto fra canale e BigScan sullo stesso segnale (QSB veloce 67,5 contro 25,7): l'avvio e l'inseguimento della velocita' della catena si
agganciano male e non sempre si riprendono. Come modo «bassa latenza» nessuna delle due varianti e' pronta: quella sulla potenza perde
171 caratteri giusti (1%) e ha il 60% di nominativi falsi in piu'; quella coerente perde il 5% dei caratteri.

## Settimo giro: velocita' in parallelo nella catena coerente

La sequenza a durate esplicite gira in otto copie (`speeds=8`): la prima segue il modello adattato, le altre stanno su una griglia di
unita' fra 10 e 55 WPM con le classi legate all'unita' (la forma del pugno imparata, spostata all'unita' dell'ipotesi). Ogni copia
accumula il suo punteggio; a ogni decisione si risale la migliore e quelle rimaste indietro di oltre 40 (logaritmo) ripartono dal suo
stato (cambio di velocita' con penalita'). Se la migliore e' su un'unita' lontana da quella del modello (oltre il 50%) per due decisioni con
toni, il modello si riaggancia a quell'unita'. Il giudizio di un tono somma a quello coerente il 30% di quello passo per passo sulla
potenza (`coh_mix=0.3`). Valori di base della catena: `emit_mode=2`, `speeds=8`, `coh_mix=0.3`, `speed_drop=40`, `relock_ln=0.4`.

Provati sulle righe difficili (3 semi, giusti): una sola velocita' 3058, otto 3216, piu' il 30% sulla potenza 3339 (con l'80%: 3306);
attuale 3409. Lasciati: varianza minima delle classi 0,04, deviazione massima 1,0, riaggancio dopo 4 decisioni o mai (il tasto a mano
migliora di poco, il salto di velocita' torna a 7,7%), 12 velocita' (+10 caratteri, calcolo +45%), otto velocita' con l'emissione sulla
potenza (la stazione vicina si rompe, 94%).

8 semi:

| totali | attuale canale | catena veloce canale | attuale BigScan | catena veloce BigScan |
|---|---|---|---|---|
| caratteri giusti | 16728 | **16769** | 16622 | **16648** |
| sbagliati / mancanti / inseriti | 893 / 661 / 290 | 1303 / 210 / 680 | 833 / 827 / 274 | 1338 / 296 / 683 |
| CER medio | **10,43** | 12,44 | **10,87** | 13,10 |
| nominativi esatti / falsi | **759 / 143** | 753 / 211 | **755 / 143** | 729 / 235 |
| falsi sul rumore (8 min) | 0 | 0 | 0 | 0 |
| latenza mediana / 95° | 1,25 / 2,13 s | **0,56 / 0,71 s** | 1,26 / 2,13 s | **0,57 / 0,73 s** |
| calcolo | **11,2 ms/s** | 12,4 ms/s | **5,7 ms/s** | 12,8 ms/s |

| CER % | attuale | catena veloce | attuale BigScan | catena veloce BigScan |
|---|---|---|---|---|
| 18-40 WPM +10, 25 WPM +3 | 0,0 | 0,1-0,2 | 0,0 | 0,1-0,6 |
| 25 WPM 0 | 1,5 | 4,3 | 1,6 | 2,4 |
| 25 WPM -3 | 20,1 | **18,7** | 21,7 | **19,0** |
| 25 WPM -6 | 81,7 | 80,6 | 86,8 | **77,4** |
| 40 WPM 0 | 8,7 | 13,1 | 11,6 | 14,7 |
| salto 18-35 | 1,1 | 3,9 | 0,6 | 7,4 |
| manuale +10 / +3 | 6,6 / 7,7 | 17,5 / 14,0 | 7,4 / 8,4 | 14,8 / 14,9 |
| qsb lento / lento 0 | 5,9 / 20,8 | 6,3 / 25,2 | 5,3 / 20,5 | 6,9 / 25,6 |
| qsb veloce | 52,4 | **51,7** | 51,1 | **47,0** |
| crepitii | 0,0 | 3,9 | 0,0 | 17,5 |
| vicina +20 | 1,1 | **0,9** | 1,1 | **0,7** |
| silenzi / 0 / 32 | 0,0 / 0,9 / 0,1 | 0,3 / 7,5 / 0,3 | 0,1 / 0,9 / 0,1 | 1,3 / 8,6 / 2,9 |

Esito: la catena veloce supera l'attuale sui caratteri giusti (+41 sul canale, +26 su BigScan) con meno della meta' della latenza, ma ha
68 nominativi falsi in piu' sul canale (92 su BigScan) e 6 esatti in meno: troppe lettere inserite (680 contro 290: evanescenza veloce,
-6 dB, tasto a mano, crepitii in BigScan). Non e' quindi ne' predefinita ne' offerta come opzione: **resta spenta**, l'interfaccia non cambia.
Mancano: meno inserzioni (sul tasto a mano e sui crepitii perde ancora molto), e il calcolo di BigScan, che con otto copie per canale
cresce con il numero dei canali.
