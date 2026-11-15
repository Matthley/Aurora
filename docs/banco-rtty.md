# Banco RTTY a livello di carattere

Prova i ricevitori RTTY su testo noto e conta i caratteri uno per uno. Non tocca i ricevitori: sta tutto in
`core/crates/mode-digi/tests/rtty_caratteri.rs` e `tests/banco_rtty/mod.rs`.

## Come si lancia

```
cd core
nice cargo test --release -p mode-digi -j 2 --test rtty_caratteri -- --ignored --nocapture --test-threads=1
```

Senza `--ignored` girano solo le tre prove veloci (generatore Baudot, allineamento, livello del rumore).

Registrazione vera (facoltativa): `AURORA_RTTY_WAV=percorso.wav` (mono o stereo, PCM 16 bit, qualunque frequenza) con la
trascrizione nello stesso percorso ma con `.txt`; `AURORA_RTTY_CENTRE` = centro fra i toni in Hz (di base 1500). Senza la
variabile il caso si salta. Sulla registrazione non c'e' il ritardo (la trascrizione non ha i tempi).

## Il segnale

ITA2 con LTRS/FIGS (cambio di registro solo quando serve, ripetuto dopo uno spazio fra le cifre), 45,45 baud, shift 170 Hz,
stop 1,5 bit, 12 kHz, centro 1500 Hz, seme fisso (ripetibile). Ogni trasmissione: 0,3 s di mark, LTRS, CR LF, testo da contest
(nominativi di prova come IU3ABC, rapporti 599 e progressivi), CR LF, 0,2 s di mark; fra le trasmissioni la portante si spegne.
S/N misurato in 2,5 kHz sulla potenza di un tono.

Scenari della prima tabella (`banco_rtty_caratteri`): AWGN da +10 a -10 dB; Rayleigh piatto lento (0,2 Hz); selettivo (mark e
space svaniscono indipendenti, 0,5 Hz); stazione a +20 dB a 250 Hz; deriva di 40 Hz lungo il minuto; silenzi di 5 s; solo rumore
per 60 s.

Seconda tabella (`banco_rtty_silenzi`), per tarare lo squelch: trasmissioni separate da 2-5 s di solo rumore, pulite, con un solo
tono che svanisce, o con crepitii (scariche di 5-50 ms, 2 al secondo).

`singolo` = `rtty::RttyDemodulator` (AFC, UOS e squelch accesi come nel programma); `multi` = `rtty_multi::RttyMulti`, il motore di
BigScan, contando i canali entro mezzo shift dal centro (sul solo rumore tutti i canali). L'audio passa a blocchi di 10 ms.

## Le colonne

Il testo atteso e quello letto si confrontano maiuscoli, con CR/LF come spazio e gli spazi ripetuti in uno solo, allineati con
Levenshtein.

- `att`: caratteri attesi; `ok`, `sbag`, `manc`, `ins`: giusti, sbagliati, mancanti, inseriti (in piu').
- `CER%`: (sbagliati + mancanti + inseriti) / attesi.
- `call ok/x/n`: nominativi letti esatti / letti sbagliati / mandati (gli altri sono persi del tutto).
- `falsi/m`: caratteri usciti nei silenzi (dopo 1 s dallo spegnimento) al minuto di silenzio; sul solo rumore, al minuto.
- `lat50`, `lat95`: ritardo in ms dei caratteri giusti, dalla fine dello stop trasmesso all'uscita (mediana e 95 %); circa 130 ms
  e' il ritardo proprio del ricevitore, le code lunghe vengono dai caratteri che escono a fine riga o dopo un riaggancio.
- `cpu`: ms di processore per secondo d'audio (un thread, `--release`).

Seconda tabella:

- `tx`: trasmissioni; `coda/min`: caratteri inseriti nel primo secondo dopo lo spegnimento della portante; `sil/min`: inseriti nel
  resto del silenzio (tutti e due al minuto di silenzio).
- `persi%`: dei primi 5 caratteri di ogni trasmissione, quanti non escono giusti.
- `primo50`, `max`: ms dall'accensione della portante al primo carattere giusto (mediana e peggiore). Circa 0,96 s sono
  fisse (0,3 s di mark, LTRS, CR, LF e il primo carattere); il resto e' il ricevitore. `mute`: trasmissioni senza nessun carattere giusto.

## Risultati 2026-11-15: presenza della portante nello squelch

Ogni carattere porta la «presenza» della portante mentre arrivava: ampiezza del tono piu' forte contro il rumore accanto ai toni (il
minore dei due canali del rumore, fondo lento con le scariche ritagliate, o la media veloce se e' piu' alta), media su 4 bit. Sul solo
rumore sta intorno a 1,7 (95 % sotto 2,1), a -10 dB il 95 % dei caratteri buoni sta sopra 2,7. Sotto 2,2 il carattere conta zero per lo
squelch (`Frame::sq_prob`): non lo apre, abbassa la media e, a squelch aperto, aspetta il seguente invece di uscire subito. Nessun
carattere si butta per i tempi: e' una prova di confidenza in piu'.

| scenario (singolo) | CER prima | CER dopo | nominativi | falsi/min prima -> dopo |
|---|---|---|---|---|
| AWGN +10 dB | 2,7 | 1,1 | 22/22 = | - |
| AWGN 0 dB | 3,8 | 0,8 | 22/22 = | - |
| AWGN -5 dB | 7,6 | 6,8 | 20/2 = | - |
| AWGN -8 dB | 18,6 | 19,0 | 13/9 -> 12/10 | - |
| AWGN -10 dB | 54,0 | 54,0 | 2/20 = | - |
| silenzi 5 s +3 dB | 3,0 | 0,8 | 22/22 = | 2,5 -> 0,0 |
| pulito +3 dB (seconda tabella) | 1,7 | 0,0 | | sil 3,8 -> 0,0 |
| un tono svanisce -3 dB | 21,3 | 20,8 | | sil 1,0 -> 0,0 |
| crepitii +3 dB | 9,9 | 6,1 | | coda 3,8 -> 0,0, sil 7,6 -> 0,0 |
| crepitii -3 dB | 7,0 | 3,6 | | sil 8,6 -> 0,0 |

Fading piatto, selettivo, vicina e deriva restano come prima; `multi` segue `singolo`. Unica perdita: a -8 dB un carattere giusto
diventa sbagliato (F5QRT letto E5QRT: la parola esce corretta in modo diverso dal modello del testo perche' cambia il flusso dei
caratteri trattenuti); con un altro seme (777) nessuna perdita e CER migliore o uguale in tutti gli scenari.

Provati e non tenuti:
- peso delle LLR abbassato durante le scariche (rumore accanto ai toni sopra 2-3 volte il fondo): crepitii un po' meglio, ma perde
  caratteri a -8/-10 dB e nel fading su un tono;
- squelch che apre a 0,5 invece di 0,7 quando i quattro caratteri hanno la portante: niente sulle prime lettere, -10 dB peggio;
- scostamento di frequenza del tono acceso (rotazione di fase media su 4 bit) oltre 6-8 Hz come prova contraria, solo con AFC o
  sintonia fine: con la vicina a +20 dB i caratteri inventati scendono da 75 a 57 (CER 39,9 -> 33,5) e i caratteri buoni stanno sempre
  entro 3 Hz, ma un carattere giusto diventa sbagliato;
- finestra di Hann sul banco di Goertzel di BigScan: la stazione accanto alla forte resta coperta dalle bande laterali della forte (il
  suo tono non e' un picco e l'anticorrelazione scende a -0,2..-0,4), il canale non si apre comunque.

## Prove 2026-11-15 (pomeriggio): sequenza a durate esplicite, verosimiglianza per tono, contrasto locale in BigScan

Nessuna tenuta. Confronti con la base fatti con 1 s di silenzio in coda all'audio (senza, la decodifica di sequenza perde l'ultimo
carattere del file solo perche' il suo ritardo e' maggiore).

1. **Decodifica di sequenza (Viterbi) sui valori morbidi**, al posto della ricerca dello start in `Frames`: passi di 1/8 di bit, stati di
   riposo a mark, start, cinque bit a mark o space, stop di durata distribuita (campana larga intorno a 1,5 bit, possibile da 0,9 a 4
   bit), ramo di silenzio che vale secondo la presenza istantanea della portante (bonus `|e| + 0,5 (2,2 - presenza)`); rilettura del
   percorso ogni 32 passi su una storia di 128 (le somme dei bit restano quelle di `Frames`, cosi' probabilita', squelch e modello del
   testo non cambiano). Il `Fuser` delle varianti deve aspettare 30 bit invece di 11.

   | scenario (singolo) | CER base | CER sequenza | giusti | nominativi |
   |---|---|---|---|---|
   | AWGN +10 dB | 1,1 | 0,8 | 263 = | 22 = |
   | AWGN -5 dB | 6,8 | 1,5 | 258 -> 263 | 20 -> 22 |
   | AWGN -8 dB | 19,0 | 17,9 | 226 -> 233 | 12 -> 14 |
   | AWGN -10 dB | 54,8 | 47,5 | 141 -> 158 | 2 = |
   | Rayleigh | 30,0 | 25,5 | 194 -> 208 | 8 -> 9 |
   | selettivo | 33,5 | 25,9 | 180 -> 203 | **8 -> 7** |
   | vicina +20 dB | 39,9 | 21,3 | 233 -> 253 | 9 -> 16 |
   | un tono svanisce -3 dB (seconda tabella) | 20,8 | 17,2 | | |
   | pulito -5 dB (seconda tabella) | 2,4 | 2,2 | | **coda 0 -> 1,0/min** |

   Con il seme 777 stesso quadro (selettivo 4 -> 6 nominativi, -8 dB 9 -> 11) ma vicina 17 -> 16 nominativi e una coda in «un tono
   svanisce +3 dB». Ritardo 134 -> 551 ms, primo carattere 1094 -> 1511 ms. Senza il ramo di silenzio i caratteri inventati nelle pause
   fra le trasmissioni salgono molto (CER +10 dB 43 % per doppioni con il `Fuser` a 11 bit, 3,8 % con il `Fuser` a 30 bit). Storia
   ridotta a 64/16 passi: molto peggio (il percorso non si e' ancora assestato). Non tenuta per un nominativo perso per seme e la coda,
   ma e' la strada che rende di piu' nel rumore e nel fading: da riprendere cercando la causa del nominativo perso.

2. **Verosimiglianza per tono.** La LLR attuale e' gia' la differenza dei logaritmi delle probabilita' dei due toni con una campana
   per livello (acceso/spento) e varianza comune. Provate due varianti:
   - ogni tono ritagliato a ±k volte la sua escursione attesa (coda pesante: un tono fuori misura non porta piu' informazione): k = 1,5
     e 1,0 peggiorano a -10 dB (141 -> 124 e 59 giusti) e nel selettivo;
   - varianza propria per tono (pesi limitati a 0,25..4): «un tono svanisce» -3 dB 20,8 -> 5,3, vicina 39,9 -> 14,4 (nominativi 9 -> 20),
     selettivo 33,5 -> 27,0, ma a +10 dB due caratteri sbagliati e un nominativo, -5 e -8 dB peggio, «pulito +3 dB» 0 -> 3,4 %. Con pesi
     0,5..2 uguale; applicata solo quando le varianze dei due toni differiscono piu' di 3 volte, il guadagno sparisce e restano piccole
     perdite. Da riprendere: la varianza per tono serve, ma la sua stima sui segnali puliti va resa stabile.

3. **BigScan, debole accanto alla forte** (scenario vicina, `multi` legge 0 caratteri):
   - contrasto locale (potenza del passo contro quelle a ±45 Hz) al posto della mediana e del picco: la vicina resta chiusa; -10 dB si
     apre (0 -> 132 giusti) ma con piu' inseriti;
   - spettro a risoluzione fine (le uscite complesse di 4 finestre di un bit sommate in fase, passi da 11 Hz) per i picchi: ora i due toni
     della debole sono picchi, ma la vicina resta chiusa perche' l'anticorrelazione si misura sulle potenze di un bit, dove la banda
     laterale della forte (a 80 Hz, circa -18 dB, quanto la debole) la porta fra -0,2 e +0,4. Con anticorrelazione richiesta solo sotto
     +0,4: vicina 0 -> 239 giusti (10 nominativi), -10 dB 0 -> 139, selettivo 164 -> 183; ma sulla registrazione `tmp30515` stazioni
     17 -> 16/26, presenza 56 -> 51 %, canali aperti in media 6 -> 16. Con soglia -0,2: registrazione 17/26 e interruzioni 3 -> 0, ma 12,7
     canali e la vicina si apre storta (55 giusti). Non tenuta: serve una prova di alternanza dei toni che non soffra le bande laterali
     della forte (filtri piu' stretti di un bit o sottrazione della forte), non una soglia piu' bassa.

La patch «scostamento del tono acceso» (prova contraria sullo scostamento di frequenza nello squelch) e' quella gia' provata e non
tenuta piu' sopra (un carattere giusto diventa sbagliato).

## 2026-11-15 (sera): decodifica di sequenza tenuta, catena probabilistica, stazioni nascoste in BigScan

Banco ora con un secondo di rumore in fondo a ogni prova (i ricevitori con piu' ritardo consegnano l'ultimo carattere),
`AURORA_RTTY_SEME` per il seme e `AURORA_STAMPA` per i testi. Tre semi: 20261115, 777, 4242.

**Tenuta, di base: decodifica di sequenza** (`rtty::Seq`, `RttyDemodulator::set_sequence_decoder`): passi di 1/8 di bit, stati di
riposo a mark, start, cinque bit, stop di durata distribuita (da 0,9 a 4 bit, campana intorno a 1,5) e silenzio, che vale quanto il
migliore dei due toni piu' 0,5 per quanto la presenza istantanea della portante sta sotto 2,2. Dal silenzio si torna solo al riposo a
mark (uno start subito dopo il silenzio inventava un carattere a ogni accensione della portante). Storia di 96 passi riletta ogni 16, il
`Fuser` delle varianti aspetta 15 bit invece di 11: ritardo 134 -> 226 ms. Storia di 64 passi: il percorso non si assesta (CER oltre il
40 %); 128 passi: 551 ms senza guadagni in piu'.

Somma sui tre semi rispetto a prima (singolo): caratteri giusti +84 / +81 / +86, inseriti -56 / -68 / -44, nominativi +11 / +6 / +13.
Perdite rimaste (una per seme): selettivo 8 -> 7 nominativi (seme del banco, una lettera dopo un FIGS letto male nel fading), vicina
17 -> 16 nominativi (seme 777), «un tono svanisce» con il seme 4242 CER 5,5 -> 7,1 e 16,4 -> 17,6, crepitii +3 dB un carattere al minuto
nella coda (semi del banco e 777). BigScan su `tmp30515`: stazioni 17 -> 16/26 (UC0A, a 1035 Hz come altre tre stazioni), presenza
56 -> 52 %, canali 6,0 = ; `multi` sul banco: -5 dB 6,8 -> 0,8, -8 dB 19,8 -> 16,0, selettivo 39,9 -> 37,3.

Provate sopra la sequenza e non tenute: varianza propria per tono (sempre o solo se le varianze differiscono piu' di 2 volte: piccoli
guadagni nei silenzi, perdite a -8 dB e nel Rayleigh); silenzio piu' forte (0,8 o 1,0 e ingresso 0,03: perde nominativi a -10 dB);
stop piu' largo (0,5 bit: peggio nella vicina).

**Ricevitore a catena probabilistica** (`rtty_chain::RttyChain`, riga `catena` del banco, non di base): rivelatore per tono (somma
mobile di un bit), inviluppo = massimo su 6 bit, livello spento = minimo su 6 bit non sotto il rumore medio, verosimiglianza a modello
misto (campana stretta di 0,6 volte il rumore, campana larga, coda esponenziale), p = L1/(L0+L1), combinazione A/(A+B), la stessa
sequenza, squelch a due livelli (errore grezzo oltre 0,2 = debole; apre con al massimo un debole su 4, chiude con 3 deboli di fila; un
debole a squelch aperto aspetta il seguente). Seme del banco, CER (giusti, nominativi):

| scenario | prima della sequenza | attuale (sequenza) | catena |
|---|---|---|---|
| AWGN +10 dB | 1,1 (263, 22) | 0,0 (263, 22) | 13,7 (255, 20) |
| AWGN 0 dB | 0,8 (263, 22) | 0,0 (263, 22) | 11,4 (258, 20) |
| AWGN -5 dB | 6,8 (258, 20) | 0,8 (263, 22) | 14,1 (253, 18) |
| AWGN -8 dB | 19,0 (226, 12) | 15,6 (234, 15) | 39,5 (194, 6) |
| AWGN -10 dB | 54,8 (141, 2) | 47,1 (160, 2) | 70,0 (80, 0) |
| Rayleigh | 30,0 (194, 8) | 25,5 (206, 9) | 28,5 (196, 8) |
| selettivo | 33,5 (180, 8) | 27,0 (199, 7) | 44,5 (176, 2) |
| vicina +20 dB | 39,9 (233, 9) | 19,8 (254, 15) | 87,1 (35, 0) |
| deriva 40 Hz | 2,3 (263, 22) | 0,0 (263, 22) | 30,0 (196, 15) |
| silenzi 5 s | 0,8 (263, 22) | 0,0 (263, 22) | 11,8 (259, 21) |
| solo rumore | 0 falsi | 0 falsi | 0 falsi |
| pulito +3 dB (2a tabella) | 0,0 | 0,0 | 9,7 |
| un tono svanisce -3 dB | 20,8 | 18,2 | 31,0 |
| crepitii -3 dB | 8,0 | 6,8 | 14,5 |

La catena da sola resta molto sotto: senza filtri stretti (la vicina passa dalla somma di un bit), senza AFC (deriva), senza varianti e
senza modello del testo. Il suo pezzo utile, la sequenza, e' gia' nel ricevitore attuale (l'ibrido: nostri filtri, LLR, squelch con la
presenza della portante, piu' la sequenza), che resta di base.

**BigScan, stazioni nascoste** (ramo `prova-bigscan-nascoste`, acceso con `AURORA_NASCOSTE`, non di base): per le coppie di toni non
trovate, uno spettro fine (Hann di 4 bit sull'ultimo secondo) per i picchi e l'anticorrelazione misurata con filtri di Hann di due bit
(bande laterali della forte a -30 dB invece di -18), solo con una stazione 10-30 volte piu' forte entro 300 Hz. Banco: vicina `multi`
0 -> 252 giusti (15 nominativi); `tmp30515`: stazioni 16/26 = , canali aperti 6,0 -> 8,3-8,5. Senza il vincolo della stazione forte:
15/26 e 10 canali. Non tenuta per i canali in piu' sulla registrazione.
