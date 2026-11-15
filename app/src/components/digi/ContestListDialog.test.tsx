import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('../../api/contestTesto', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/contestTesto')>()),
  getTextContests: vi.fn(() => Promise.resolve({ attivo: 'B', contest: [{ id: 'A', name: 'Alfa RTTY', notes: 'RTTY', start: '', end: '' }, { id: 'B', name: 'Beta PSK', notes: 'PSK63', start: '', end: '' }], macros: [], domini: [] })),
  selectTextContest: vi.fn(() => Promise.resolve({ ok: true, attivo: 'A' })),
  deleteTextContest: vi.fn(() => Promise.resolve({ ok: true })),
  importTextContest: vi.fn(() => Promise.resolve({ ok: true, id: 'Gamma' })),
  exportTextContest: vi.fn(() => Promise.resolve(new Blob(['PK']))),
}));
vi.mock('../ConfirmHost', () => ({ askConfirm: vi.fn(() => Promise.resolve(true)) }));
import { deleteTextContest, exportTextContest, importTextContest, selectTextContest } from '../../api/contestTesto';
import { ContestListDialog, fromLocalInput, toLocalInput } from './ContestListDialog';

const dialog = (onClose = vi.fn()) => (
  <QueryClientProvider client={new QueryClient()}>
    <ContestListDialog onClose={onClose} />
  </QueryClientProvider>
);

beforeEach(() => {
  vi.mocked(selectTextContest).mockClear();
  vi.mocked(deleteTextContest).mockClear();
});

describe('elenco dei contest', () => {
  it('prima riga «Nessun contest», freccia su quello attivo, doppio clic seleziona', async () => {
    const onClose = vi.fn();
    render(dialog(onClose));
    const alfa = await screen.findByText('Alfa RTTY');
    const rows = screen.getAllByRole('row');
    // titolo, «Nessun contest», Alfa, Beta
    expect(rows).toHaveLength(4);
    expect(rows[3]).toHaveTextContent('►');
    expect(rows[2]).not.toHaveTextContent('►');
    await act(async () => {
      fireEvent.doubleClick(alfa);
    });
    expect(selectTextContest).toHaveBeenCalledWith('A');
    expect(onClose).toHaveBeenCalled();
  });

  it('Elimina chiede conferma e cancella la riga scelta; «Nessun contest» non si elimina', async () => {
    render(dialog());
    const beta = await screen.findByText('Beta PSK');
    fireEvent.click(beta);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Elimina|Delete/ }));
    });
    expect(deleteTextContest).toHaveBeenCalledWith('B');
    fireEvent.click(screen.getAllByRole('row')[1] as HTMLElement);
    expect(screen.getByRole('button', { name: /Elimina|Delete/ })).toBeDisabled();
  });

  it('Importa manda il file scelto, Esporta scarica il pacchetto del contest scelto', async () => {
    URL.createObjectURL = vi.fn(() => 'blob:x');
    URL.revokeObjectURL = vi.fn();
    render(dialog());
    const alfa = await screen.findByText('Alfa RTTY');
    const file = new File(['[Contest]'], 'Gamma.zip');
    await act(async () => {
      fireEvent.change(screen.getByTestId('ctest-import-file'), { target: { files: [file] } });
    });
    expect(importTextContest).toHaveBeenCalledWith(file);
    fireEvent.click(alfa);
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /Esporta contest|Export contest/ }));
    });
    expect(exportTextContest).toHaveBeenCalledWith('A');
  });

  it('date del file e campo data/ora', () => {
    expect(toLocalInput('20150926000001')).toBe('2015-09-26T00:00:01');
    expect(toLocalInput('')).toBe('');
    expect(fromLocalInput('2015-09-26T00:00')).toBe('20150926000000');
    expect(fromLocalInput('')).toBe('');
  });
});
