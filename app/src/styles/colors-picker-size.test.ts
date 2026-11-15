import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';

// i fogli di stile si leggono dal disco (le prove girano dalla cartella `app`): nelle prove gli `import` dei CSS sono vuoti
const css = readFileSync('src/styles/settings-window.css', 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
const rule = (selector: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].find((m) => (m[1] ?? '').trim() === selector)?.[2] ?? '';

describe('Colori e anteprima nella finestra Impostazioni (issue #70)', () => {
  it('il selettore del colore ha una misura fissa e non viene schiacciato', () => {
    const body = rule(".sw-panel .jt-colors .sx-field input[type='color']");
    expect(body).toMatch(/flex:0 0 auto/);
    expect(body).toMatch(/min-width:1\.8rem/);
  });

  it('la tendina del suono resta stretta (la regola generale la allarga al 100%)', () => {
    expect(rule('.sw-panel .jt-colors .sx-field select.snd-select')).toMatch(/width:6\.2rem/);
  });
});
