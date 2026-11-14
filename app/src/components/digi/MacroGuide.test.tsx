import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLang } from '../../i18n';
import { HELP_CATEGORIES, MACRO_COMMANDS, MACRO_EVENTS } from '../../lib/macroCommands';
import { MacroGuide } from './MacroGuide';

const tag = (name: string) => screen.queryAllByText(name, { selector: 'code.mguide-tag' });

describe('Guida alle macro', () => {
  beforeEach(() => useLang.setState({ lang: 'it' }));

  it('ogni comando e ogni macro evento si raggiunge dalla sua categoria', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Guida alle macro' })).toBeTruthy();
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(HELP_CATEGORIES.length + 1);
    HELP_CATEGORIES.forEach((cat, i) => {
      const tab = tabs[i];
      if (!tab) throw new Error(cat);
      fireEvent.click(tab);
      expect(tab.getAttribute('aria-selected')).toBe('true');
      for (const c of MACRO_COMMANDS.filter((m) => m.cat === cat)) expect(tag(c.tag).length, c.tag).toBeGreaterThan(0);
    });
    const last = tabs.at(-1);
    if (!last) throw new Error('eventi');
    fireEvent.click(last);
    for (const e of MACRO_EVENTS) expect(tag(e.name).length, e.name).toBeGreaterThan(0);
  });

  it('frecce su e giu cambiano categoria', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    const [first, second] = screen.getAllByRole('tab');
    if (!first || !second) throw new Error('mancano le categorie');
    fireEvent.keyDown(first, { key: 'ArrowDown' });
    expect(second.getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(second, { key: 'ArrowUp' });
    expect(first.getAttribute('aria-selected')).toBe('true');
  });

  it('il filtro cerca in tutte le categorie', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'rotore' } });
    expect(tag('<ROTOR:SP>').length).toBeGreaterThan(0);
    expect(tag('<MYCALL>')).toHaveLength(0);
    expect(screen.getAllByRole('tab').length).toBeLessThan(HELP_CATEGORIES.length + 1);
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'zzzz' } });
    expect(screen.getByRole('status').textContent).toBe('Nessun comando trovato.');
  });

  it('un clic apre la spiegazione lunga sotto la riga', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    const withLong = MACRO_COMMANDS.find((c) => c.long && c.cat === HELP_CATEGORIES[0]) ?? MACRO_COMMANDS.find((c) => c.cat === HELP_CATEGORIES[0]);
    if (!withLong) throw new Error('nessun comando');
    const line = tag(withLong.tag)[0]?.closest('button');
    if (!line) throw new Error('nessuna riga');
    expect(line.getAttribute('aria-expanded')).toBe('false');
    fireEvent.click(line);
    expect(line.getAttribute('aria-expanded')).toBe('true');
    const li = line.closest('li');
    if (!li) throw new Error('nessun li');
    expect(within(li).getByText(withLong.ex, { selector: 'code.mguide-full' })).toBeTruthy();
  });

  it('in olandese senza voce del dizionario usa l\'inglese; Esc chiude', () => {
    useLang.setState({ lang: 'nl' });
    const onClose = vi.fn();
    render(<MacroGuide onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('searchbox'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
