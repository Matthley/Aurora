import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../api/client', () => ({ putBandFilters: vi.fn(async () => undefined) }));

import { ActivityTagPills } from './ActivityTagPills';
import { defaultFilters, useFilters } from '../lib/filters';

describe('ActivityTagPills', () => {
  beforeEach(() => useFilters.setState({ filters: defaultFilters() }));

  it('hides a continent from the Filter popup', () => {
    render(<ActivityTagPills />);
    const eu = screen.getByRole('button', { name: 'EU' });
    expect(eu).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(eu);
    expect(useFilters.getState().filters.hideContinents).toEqual(['EU']);
    expect(screen.getByRole('button', { name: 'EU' })).toHaveAttribute('aria-pressed', 'true');
  });

  it('hides countries and callsigns typed in the Filter popup', () => {
    render(<ActivityTagPills />);
    const [countries, calls] = screen.getAllByRole('textbox');
    fireEvent.change(countries!, { target: { value: 'Japan, Brazil' } });
    fireEvent.blur(countries!);
    fireEvent.change(calls!, { target: { value: 'ja*, iu3abc' } });
    fireEvent.blur(calls!);
    const f = useFilters.getState().filters;
    expect(f.hideCountries).toEqual(['Japan', 'Brazil']);
    expect(f.hideCalls).toEqual(['JA*', 'IU3ABC']);
  });
});
