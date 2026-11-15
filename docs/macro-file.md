# File delle macro dei modi testuali

Le macro dei modi testuali (PSK31, PSK63, RTTY, CW) stanno nella cartella dei dati di Aurora (`%APPDATA%\Aurora`), sottocartella `macro/`:

| File | Contenuto |
|---|---|
| `comuni.macro` | macro comuni a tutti i modi |
| `psk31.macro`, `psk63.macro`, `rtty.macro`, `cw.macro` | macro solo di quel modo (il file manca se il modo non ne ha) |

L'impostazione «Macro di questo modo + macro comuni» (`merge_macros` in `digi.json`) dice come si combinano:
acceso, valgono le macro comuni e quelle del modo sostituiscono i tasti con lo stesso nome; spento, valgono solo quelle del modo.

## Formato

Testo semplice (UTF-8), lo stesso dei file di macro dei contest (`contest/macros/*.mc`), con il colore leggibile:

```ini
; righe che cominciano con ; o # sono commenti
[Macros]
nItems=2

Name0=F2
Label0=CQ
Text0=<TX>CQ CQ CQ DE <MYCALL> <MYCALL> K<RX>
Color0=#FFFF80

Name1=Ctrl-F5
Label1=Call
Text1=<CALL>
```

- `NameN`: il tasto (`F1`…`F12`, con `Ctrl-`, `Shift-`, `Ctrl-Shift-`) oppure un evento (`OnStart`, `OnEnd`, `OnTX`, `OnCR`, `OnClickAny`, `OnAlarm`, `OnSP0`, `OnSP1`, `Gray+`, `Hello`, `AutoCqText`, `OM`).
- `LabelN`: la scritta del pulsante (al massimo 60 caratteri).
- `TextN`: il testo della macro su una riga; gli a capo si scrivono `<CR><LF>`.
- `ColorN`: lo sfondo del pulsante `#RRGGBB` (facoltativo). Si legge anche `BkColorN`, il colore di Windows in decimale (`0xBBGGRR`) dei file `.mc`.
- `nItems` e' informativo: contano tutte le voci numerate con un nome, in ordine di numero. Le voci con un nome non valido si saltano.

I file si possono modificare a mano con Aurora chiusa: Aurora li riscrive a ogni salvataggio dalla pagina delle macro.

## Migrazione da `digi.json`

Fino alla 0.11.266 le macro stavano in `digi.json` (`macros`, `mode_macros`). Al primo avvio senza `macro/comuni.macro` Aurora:

1. copia `digi.json` in `digi.json.prima-dei-file-macro.bak`;
2. scrive le macro nei file di `macro/`;
3. riscrive `digi.json` senza `macros` e `mode_macros` (`merge_macros` resta).

Per ripartire dalle macro di prima basta cancellare la cartella `macro/` e rimettere la copia di sicurezza al posto di `digi.json`.

## Formato .mc importato

L'esempio `core/crates/mode-digi/examples/importa_mc.rs` converte una cartella di file di macro nel formato `.mc` importato nei file `.macro`
di Aurora e stampa un rapporto (quante macro per file, comandi non tradotti):

```
cargo run -p mode-digi --example importa_mc -- <cartella .mc> <cartella di uscita>
```
