import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Macro } from '../lib/macroLang';
import { MacroSlots, OPEN_ROWS_KEY, readOpenRows, slotChip, slotName, writeOpenRows } from './MacroSlots';

const mk = (name: string, label: string, text: string, color: number | null = null): [string, Macro] => [name, { name, label, text, color }];

describe('macro slots helpers', () => {
  it('maps rows to the stored names and the chips', () => {
    expect(slotName(0, 1)).toBe('F1');
    expect(slotName(1, 5)).toBe('Ctrl-F5');
    expect(slotName(2, 12)).toBe('Shift-F12');
    expect(slotName(3, 3)).toBe('Ctrl-Shift-F3');
    expect(slotChip(0, 7)).toBe('F7');
    expect(slotChip(2, 7)).toBe('7');
  });

  it('opens only row 1 by default and remembers the choice', () => {
    const map = new Map<string, string>();
    const store = { getItem: (k: string) => map.get(k) ?? null, setItem: (k: string, v: string) => void map.set(k, v) };
    expect(readOpenRows(store)).toEqual([true, false, false, false]);
    writeOpenRows(store, [false, true, false, true]);
    expect(map.has(OPEN_ROWS_KEY)).toBe(true);
    expect(readOpenRows(store)).toEqual([false, true, false, true]);
    map.set(OPEN_ROWS_KEY, 'rotto');
    expect(readOpenRows(store)).toEqual([true, false, false, false]);
  });
});

describe('MacroSlots', () => {
  beforeEach(() => localStorage.clear());

  const macros = new Map([mk('F1', 'CQ', 'CQ CQ DE IU3ABC\nK', 0xff0000), mk('Ctrl-F2', 'TU', 'TU 73')]);

  it('shows four groups, twelve slots in the open one, and edits by stored name', () => {
    const onEdit = vi.fn();
    render(<MacroSlots macros={macros} scopeOf={(n) => (n === 'F1' ? 'mode' : 'all')} modeName="RTTY" onEdit={onEdit} />);
    const heads = screen.getAllByRole('button', { expanded: false }).concat(screen.getAllByRole('button', { expanded: true }));
    expect(heads).toHaveLength(4);
    const items = document.querySelectorAll('.sw-mslots li');
    expect(items).toHaveLength(12);
    const f1 = items[0] as HTMLElement;
    expect(within(f1).getByText('F1')).toBeTruthy();
    expect(within(f1).getByText('RTTY')).toBeTruthy();
    expect(f1.querySelector('.sw-mslot-text')?.getAttribute('title')).toBe('CQ CQ DE IU3ABC\nK');
    expect(items[1]?.classList.contains('sw-mslot--empty')).toBe(true);
    fireEvent.click(within(items[1] as HTMLElement).getByRole('button'));
    expect(onEdit).toHaveBeenCalledWith('F2');
  });

  it('opens row 2 on click, shows positions and remembers it', () => {
    const onEdit = vi.fn();
    render(<MacroSlots macros={macros} scopeOf={() => 'all'} modeName="CW" onEdit={onEdit} />);
    const row2 = document.querySelector('[data-row="2"] .sw-mrow-head') as HTMLElement;
    fireEvent.click(row2);
    expect(row2.getAttribute('aria-expanded')).toBe('true');
    const slot = document.querySelector('[data-name="Ctrl-F2"]') as HTMLElement;
    expect(within(slot).getByText('2')).toBeTruthy();
    fireEvent.click(within(slot).getByRole('button'));
    expect(onEdit).toHaveBeenCalledWith('Ctrl-F2');
    expect(JSON.parse(localStorage.getItem(OPEN_ROWS_KEY) ?? 'null')).toEqual([true, true, false, false]);
  });
});
