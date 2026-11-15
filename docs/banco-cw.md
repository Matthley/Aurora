# Banco del ricevitore CW a caratteri

Prova `core/crates/mode-digi/tests/banco_cw.rs`: misura il ricevitore CW di Aurora carattere per carattere, prima e dopo ogni modifica,
sul canale singolo (`CwDemodulator` al tono giusto, impostazioni di base del servizio) e su BigScan CW (`CwBandDecoder` su tutta la banda:
si raccoglie il testo dei canali entro 25 Hz dal tono).

```
CARGO_BUILD_JOBS=2 nice cargo test --release -j 2 -p mode-digi --test banco_cw -- --ignored --nocapture --test-threads=1
```

Circa 75 secondi. Variabili: `CW_BANCO_SEMI` (semi per scenario, di base 3), `CW_RIGHE` (scenari il cui nome contiene uno dei pezzi,
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
- **call**: nominativi mandati ritrovati esatti come parola, nell'ordine; **falsi**: parole ricevute che hanno la forma di un nominativo
  ma non sono fra quelli mandati.
- **falsi/min** sul solo rumore: caratteri non spazio stampati al minuto (BigScan: da tutti i canali).
- **lat**: mediana e 95° percentile del tempo fra la fine del carattere trasmesso e la sua stampa (l'audio entra a blocchi di 100 ms).
- **cpu**: millisecondi di calcolo per secondo d'audio (un thread; per BigScan tutto il decodificatore della banda).

## Risultati

Codice di partenza (commit del banco), 3 semi per scenario. CER in %:

| scenario | canale | BigScan |
|---|---|---|
| 18/25/32/40 WPM +10 | 0,0 / 0,0 / 0,0 / 0,0 | 0,0 / 0,0 / 0,0 / 0,0 |
| 25 WPM +3 / 0 / -3 / -6 | 0,0 / 4,9 / 45,3 / 83,8 | 0,0 / 4,9 / 32,4 / 89,0 |
| 40 WPM 0 | 17,5 | 18,7 |
| salto 18-35 | 0,9 | 0,9 |
| manuale +10 / +3 (ns) | 24,0 (5,0) / 26,9 (7,2) | 24,4 (5,4) / 27,2 (6,3) |
| qsb lento / lento 0 | 9,8 / 32,7 | 8,9 / 31,8 |
| qsb veloce | 58,1 | 58,1 |
| crepitii | 0,0 | 0,0 |
| vicina +20 | 1,8 | 2,1 |
| silenzi / 0 / 32 | 0,0 / 0,9 / 0,0 | 0,0 / 0,6 / 0,0 |
| rumore 60 s (falsi/min) | 0,0 | 0,0 |
| **CER medio per scenario** | **15,33** | **14,95** |

Nominativi esatti 244/350 (canale), 242/350 (BigScan); nominativi falsi 65 e 69; latenza mediana 1,17 s, 95° 2,07 s; calcolo 13,6 ms/s
(canale) e 6,9 ms/s (BigScan, che apre il canale solo quando vede il segnale).
