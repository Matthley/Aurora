# Regole dei contest dei modi testuali

Riassunto dei regolamenti ufficiali usati per la sezione `[Score]` dei file `.contest` (`docs/contest-file.md`), con la fonte e la
data di lettura. Il punteggio di Aurora e' una stima: quello ufficiale lo calcola l'organizzatore dal Cabrillo.
Le prove con i log calcolati a mano stanno in `core/crates/contest/tests/regole_contest.rs`.

## Primo gruppo (letti il 2026-11-15)

### CQ WW RTTY DX Contest
Fonte: <https://www.cqwwrtty.com/rules.htm> (CQ).
- Bande 80, 40, 20, 15, 10 m. Scambio RST + zona CQ; W e VE aggiungono stato o area.
- Punti: stesso paese 1, stesso continente 2, altro continente 3.
- Moltiplicatori per banda: zone CQ, paesi (DXCC + WAE), stati USA (48 + DC) e aree canadesi (14). KL7 e KH6 contano solo come paese.
- Punteggio = punti x (zone + paesi + QTH W/VE).
- `[Score]`: `points=same_dxcc=1; same_cont=2; other_cont=3`, `mult=cq/band; dxcc/band; exch/band/if:dxcc:K,VE`.

### CQ WPX RTTY Contest
Fonte: <https://www.cqwpxrtty.com/rules.htm> (CQ).
- Bande 80-10 m. Scambio RST + numero.
- Punti su 28/21/14 MHz: altro continente 3, stesso continente 2, stesso paese 1; su 7 e 3,5 MHz il doppio.
- Moltiplicatori: prefissi WPX, una volta sola in tutto il contest.
- `[Score]`: `points_band=80:2,40:2`, `mult=wpx`.

### WAE DX Contest RTTY (DARC)
Fonte: <https://www.darc.de/der-club/referate/conteste/wae-dx-contest/en/wae-rules/> (DARC, §6, §7, §8, §12).
- Bande 80-10 m. In RTTY tutti lavorano tutti; i QTC solo fra continenti diversi, al massimo 10 fra due stazioni (`docs/contest-qtc.md`).
- Punti: 1 per QSO, 1 per QTC mandato o ricevuto.
- Moltiplicatori per banda (§6, che vale anche in RTTY): paesi DXCC ed elenco WAE; per W, VE, VK, ZL, ZS, JA, BY, PY, RA8, RA9 e RA0
  contano le aree numeriche. Peso: 80 m x4, 40 m x3, 20/15/10 m x2.
- Punteggio = (QSO + QTC) x somma dei moltiplicatori pesati.
- `[Score]`: `mult=dxcc/band/except:K,VE,VK,ZL,ZS,JA,BY,PY,UA9; area:K,VE,VK,ZL,ZS,JA,BY,PY,UA9/band`, `mult_band=80:4,40:3,20:2,15:2,10:2`;
  `m_iQtc=2`.

### BARTG HF RTTY Contest
Fonte: <https://bartg.org.uk/wp-content/uploads/2025/05/bartg-hf-rtty-rules-2025-v3.pdf> (BARTG, regole 2025 v3).
- Bande 80-10 m, con i segmenti di frequenza del regolamento (fuori segmento zero punti: non controllato).
- Punti: 1 per QSO, una volta per banda.
- Moltiplicatori per banda: paesi DXCC (anche JA, W, VE, VK) e aree numeriche JA, W, VE, VK. Continenti: una volta sola, al massimo 6.
- Punteggio = punti x moltiplicatori x continenti.
- `[Score]`: `mult=dxcc/band; area:JA,K,VE,VK/band; cont/product`.

### ARRL RTTY Roundup
Fonte: <https://www.arrl.org/rtty-roundup> e il regolamento completo
<https://arrl.org/files/file/Contest%20Rules%20PDFs/2022/RTTY%20Roundup%20Rules%20-%202_2%20-%202022.pdf> (ARRL, versione 2.2).
- Bande 80-10 m, una volta per banda in qualsiasi modo. Scambio: W/VE RST + stato o provincia, gli altri RST + numero.
- Punti: 1 per QSO.
- Moltiplicatori una volta sola: stati USA + DC, province e territori canadesi, paesi DXCC esclusi USA e Canada (KH6 e KL7 sono DXCC).
- `[Score]`: `mult=exch/if:dxcc:K,VE; dxcc/except:K,VE`; `m_bDupeNoMode=1`.

### RAEM (Coppa della Russia «Memorial E. Krenkel»)
Fonte: <https://raem.srr.ru/?p=47> (SRR), confrontato con l'elenco di <https://www.digitalrus.ru/>.
- Scambio: numero + latitudine e longitudine in gradi interi (`57N85E`).
- Punti: 50 per QSO; 1 punto per grado di differenza di latitudine e 1 per grado di longitudine; +100 con una stazione polare
  (latitudine 66 o piu'); +300 con la stazione commemorativa RAEM. Nessun moltiplicatore; chi sta dentro i circoli polari moltiplica il
  totale per 1,1.
- `[Score]`: `points=any=50`, `bonus=call:RAEM=300`; `[Script]`: `punti(qso)` con le coordinate e la stazione polare,
  `punteggio(totali)` con il fattore 1,1 (esempio in `docs/contest-file.md`).

### SARTG WW RTTY Contest
Fonte: <https://www.sartg.com/contest/wwrules.htm> (SARTG).
- Bande 80-10 m. Scambio RST + numero.
- Punti: stesso paese 5, stesso continente 10, altro continente 15.
- Moltiplicatori per banda: paesi DXCC e aree numeriche di VK, VE, JA e W.
- `[Score]`: `points=same_dxcc=5; same_cont=10; other_cont=15`, `mult=dxcc/band; area:K,VE,JA,VK/band`.

### Russian WW Digital Contest
Fonte: <http://www.rdrclub.ru/index.php/russian-ww-digital-contest/51-rus-ww-digi-rules> (Russian Digital Radio Club), trovato
tramite <https://www.digitalrus.ru/>.
- RTTY45 e BPSK63, bande 160-10 m; 3-4 ottobre 2026, 12:00-11:59 UTC. Scambio: russi RST + oblast, gli altri RST + numero.
- Punti su 20/15/10 m: stesso paese 1, altro paese 3, altro continente 5 (stazione QRP 5); su 160/80/40 m il doppio.
- Moltiplicatori per banda e per modo: paesi DXCC e oblast russi.
- `[Score]`: `points_band=160:2,80:2,40:2`, `mult=dxcc/band/mode; exch/band/mode/if:dxcc:UA,UA9,UA2` (oblast dall'elenco
  `dom/russian.dom`). Il file conteneva i dati del Russian WW PSK: nome, note e date sono stati corretti.

### Ukrainian DX DIGI Contest
Fonte: riassunto del regolamento su <https://www.qsl.net/contest_ari/reg_ukr_digi.html> (ARI), che rimanda a
<http://www.qsl.net/ur5fav/udrpc/>; sito del contest indicato da <https://www.digitalrus.ru/>: <http://www.izmail-dx.com/> (non
raggiungibile il giorno della lettura).
- RTTY e PSK31, bande 80-10 m; una stazione si lavora una volta per banda e per modo. Scambio: ucraini RST + oblast, gli altri RST + numero.
- Punti: stesso continente 1, altro continente 3, stazioni ucraine 5; su 80 m il doppio.
- Moltiplicatori per banda e per modo: paesi DXCC e oblast ucraini.
- `[Score]`: `points=dxcc:UR=5; same_cont=1; other_cont=3`, `points_band=80:2`, `mult=dxcc/band/mode; exch/band/mode/if:dxcc:UR`.

## Secondo gruppo (letti il 2026-11-15)

### DL-DX RTTY Contest (DRCG)
Fonte: il sito della DRCG (<http://drcg.de/>, indicato da <https://www.digitalrus.ru/>) non e' piu' raggiungibile; regole dal riassunto
ARI <https://www.qsl.net/contest_ari/reg_dldx_rtty.html> (edizione 2005). Secondo <https://www.ig-ry.de/contest-cup> i contest DRCG di
giugno e luglio sono stati sospesi per sempre: il file resta per chi rilegge i vecchi log.
- RTTY, PSK31, PSK63, bande 80-10 m. Scambio RST + numero.
- Punti: stesso paese 5, stazione DL 8, altro paese europeo 10, fuori Europa 15.
- Moltiplicatori per banda: paesi DXCC e aree numeriche di W, VE, JA, VK (il primo QSO con questi paesi vale doppio).
- `[Score]`: `points=same_dxcc=5; dxcc:DL=8; cont:EU=10; any=15`, `mult=dxcc/band; area:K,VE,JA,VK/band`.

### SP DX RTTY Contest (PK RVG)
Fonte: il sito indicato da <https://www.digitalrus.ru/> (<http://www.pkrvg.org/zbior.html>) non e' raggiungibile; regole dal riassunto ARI
<https://www.qsl.net/contest_ari/reg_spdxrtty.html>.
- Bande 80-10 m. Scambio RST + numero; le stazioni polacche RST + provincia (una lettera).
- Punti: stesso paese 2, stesso continente 5, altro continente 10.
- Moltiplicatori per banda: paesi DXCC e province polacche (16); continenti una volta sola (al massimo 6), a parte.
- Punteggio = punti x (paesi + province) x continenti.
- `[Score]`: `points=same_dxcc=2; same_cont=5; other_cont=10`, `mult=dxcc/band; exch/band/if:dxcc:SP; cont/product`.

### OK DX RTTY Contest (CRK)
Fonte: <http://www.crk.cz/ENG/DXCONTE.HTM> (CRK, indicato da <https://www.digitalrus.ru/>; il server rifiuta la lettura automatica),
regole 2020 riportate da <https://cq.sk/en/ok-dx-rtty-contest-2020>.
- Bande 80-10 m. Scambio RST + zona CQ.
- Punti: su 10/15/20 m stesso continente 1, gli altri 2; su 40/80 m stesso continente 3, gli altri 6.
- Moltiplicatori per banda: paesi DXCC; per chi non e' OK/OL anche ogni stazione OK diversa.
- `[Score]`: `points=band:80,40&same_cont=3; band:80,40=6; same_cont=1; any=2`, `mult=dxcc/band; call/band/if:dxcc:OK&not_my_dxcc:OK`.

### EA RTTY Contest (URE)
Fonte: <https://concursos.ure.es/en/eartty/bases/> (URE).
- Bande 80-10 m. Scambio: EA RST + provincia, gli altri RST + numero; EA4URE manda HQ.
- Punti: EA con EA (anche EA6, EA8, EA9) 2, EA con DX 1, DX con DX 1, DX con EA 3.
- Moltiplicatori per banda: entita' EADX-100, province spagnole, EA4URE (HQ), aree numeriche di W, VE, JA, VK (il primo QSO con questi
  paesi e con EA, EA6, EA8, EA9 vale doppio: paese e area o provincia).
- `[Score]`: `points=my_dxcc:EA,EA6,EA8,EA9&dxcc:EA,EA6,EA8,EA9=2; my_dxcc:EA,EA6,EA8,EA9=1; dxcc:EA,EA6,EA8,EA9=3; any=1`,
  `mult=dxcc/band; exch/band/if:dxcc:EA,EA6,EA8,EA9; area:K,VE,JA,VK/band`.

### Mexican (FMRE) RTTY International Contest
Fonte: <https://rtty.fmre.mx/reglas/Rules_FMRE_RTTY_2024.pdf> (FMRE, 26a edizione, 2024).
- Bande 80-10 m. Scambio: XE RST + stato, gli altri RST + numero.
- Punti: con una stazione XE 4, stesso paese 2, altro paese 3.
- Moltiplicatori per banda: i 32 stati messicani (elenco `dom/Mexico32.dom`) e i paesi DXCC.
- `[Score]`: `points=dxcc:XE=4; same_dxcc=2; any=3`, `mult=exch/band/if:dxcc:XE; dxcc/band`; `m_sStateListFile=dom/Mexico32.dom`.

## Cosa resta approssimato

- Zone CQ e ITU vengono dal database DXCC di Aurora, non dallo scambio ricevuto (un W6 che passa zona 3 conta la zona del paese).
- Elenco WAE (IT9, GM/Shetland, JW/Bear, 4U1V...) non distinto dal DXCC; nel WAE RTTY contano tutti i paesi per tutti.
- Stazioni /MM, QRP (5 punti nel Russian WW Digital) e i segmenti di frequenza del BARTG non sono riconosciuti.
- Stati, province e oblast si prendono dallo scambio cosi' com'e' scritto: senza elenco dei domini non si controlla che esistano.
