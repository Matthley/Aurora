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
