import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

// i fogli di stile si leggono dal disco (le prove girano dalla cartella `app`): nelle prove gli `import` dei CSS sono vuoti
// i fogli che disegnano il display della radio e la scritta «radio spenta»
const FILES = ['components.css', 'radiodisplay.css', 'modern-displays.css', 'onair.css', 'bozze.css', 'solo.css', 'first-run.css'];
const sheets = FILES.map((f) => [f, readFileSync(`src/styles/${f}`, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '')] as const);

/** Le regole (selettore e dichiarazioni) di tutti i fogli. */
function rules(): Array<{ sheet: string; selector: string; body: string }> {
  return sheets.flatMap(([sheet, css]) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].map((m) => ({ sheet, selector: (m[1] ?? '').trim(), body: m[2] ?? '' })));
}

describe('display della radio in testata', () => {
  // issue #55 e #60: il display in testata sta fuori dal flusso e prende la larghezza dal contenuto. Con `container-type: inline-size` la
  // larghezza non dipende piu' dal contenuto: il display diventava largo zero e spariva con qualunque stile (S-meter e dati in piu' compresi)
  it('never gets size containment on the display boxes', () => {
    const boxes = /\.rd-(header|square-slot|square-preview|zoom-host|with-rig|duo|face-row|modern-slot)\b/;
    const bad = rules().filter((r) => /container-type\s*:\s*(inline-size|size)/.test(r.body) && r.selector.split(',').some((s) => boxes.test(s.trim().split(/\s+/).pop() ?? '')));
    expect(bad.map((r) => `${r.sheet}: ${r.selector}`)).toEqual([]);
  });

  it('sizes the radio-off text without container units', () => {
    const bad = rules().filter((r) => /rd-radio-off/.test(r.selector) && /\d(cqi|cqw|cqh)\b/.test(r.body));
    expect(bad.map((r) => `${r.sheet}: ${r.selector}`)).toEqual([]);
  });
});
