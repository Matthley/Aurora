import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useLang } from '../../i18n';
import { MACRO_COMMANDS } from '../../lib/macroCommands';
import { MacroGuide } from './MacroGuide';

describe('Guida alle macro', () => {
  beforeEach(() => useLang.setState({ lang: 'it' }));

  it('mostra tutti i comandi dell\'elenco e le macro evento', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    expect(screen.getByRole('dialog', { name: 'Guida alle macro' })).toBeTruthy();
    for (const c of MACRO_COMMANDS) expect(screen.getAllByText(c.tag, { selector: 'dt > code' }).length, c.tag).toBeGreaterThan(0);
    expect(screen.getByText('OnTX', { selector: 'dt > code' })).toBeTruthy();
  });

  it('il filtro cerca nei comandi e nelle spiegazioni', () => {
    render(<MacroGuide onClose={vi.fn()} />);
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'rotore' } });
    expect(screen.getByText('<ROTOR:SP>', { selector: 'dt > code' })).toBeTruthy();
    expect(screen.queryByText('<MYCALL>', { selector: 'dt > code' })).toBeNull();
    fireEvent.change(screen.getByRole('searchbox'), { target: { value: 'zzzz' } });
    expect(screen.getByRole('status').textContent).toBe('Nessun comando trovato.');
  });

  it('in olandese senza voce del dizionario usa l\'inglese; Esc chiude', () => {
    useLang.setState({ lang: 'nl' });
    const onClose = vi.fn();
    render(<MacroGuide onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('searchbox'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });
});
