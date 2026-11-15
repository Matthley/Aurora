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
