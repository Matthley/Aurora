import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SstvSnapshot } from '../../api/client';

vi.mock('../../api/client', () => ({
  ApiError: class ApiError extends Error {},
  deleteSstvTxPhotos: vi.fn(() => Promise.resolve(0)),
  getSstvTxFolder: vi.fn(() => Promise.resolve({ files: [] })),
  getSstvTxPhoto: vi.fn(),
  sstvTxPhotoUrl: vi.fn((n: string) => `/photo/${n}`),
  sstvAbort: vi.fn(() => Promise.resolve()),
  sstvTransmit: vi.fn(() => Promise.resolve()),
}));
// jsdom non disegna: la foto "caricata" e' una tela finta e le misure dei livelli coprono tutta l'immagine (ogni clic prende il livello in cima)
vi.mock('../../lib/sstvRender', () => ({
  boxPoint: () => ({ x: 160, y: 128 }),
  fontString: () => '32px sans-serif',
  layerBox: () => ({ cx: 160, cy: 128, w: 10000, h: 10000 }),
  loadImageFile: vi.fn(() => Promise.resolve(document.createElement('canvas'))),
  renderComposition: vi.fn(),
}));
import { Composer } from './Composer';

const SNAP: SstvSnapshot = {
  rx: { listening: true, receiving: false, mode: null, width: 320, height: 256, rows: 0, sync: 0, offset_hz: 0, level_db: -40, last_image: null },
  tx: { state: 'idle', mode: null, progress: 0, elapsed_s: 0, total_s: 0, message: null },
  settings: { default_mode: 'm1', auto_save: true, save_tx: false, auto_receive: false, lock_mode: null, folder: '', tx_folder: '', rx_offset_hz: 0, tx_offset_hz: 0, slant_ppm: 0 },
  folder: 'C:\\Aurora\\history',
  can_transmit: true,
  modes: [{ id: 'm1', name: 'Martin 1', width: 320, height: 256, vis: 44, seconds: 114, color: 'rgb' }],
};

function composer() {
  return render(
    <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
      <Composer snap={SNAP} incoming={null} onChanged={() => undefined} />
    </QueryClientProvider>,
  );
}

// contesto 2D finto: ogni metodo e' una funzione che non fa nulla, ogni proprieta' si puo' scrivere
const fakeContext = () => new Proxy({} as Record<string, unknown>, { get: (o, k: string) => (k in o ? o[k] : () => undefined), set: (o, k: string, v) => ((o[k] = v), true) }) as unknown as CanvasRenderingContext2D;
const tab = (name: string) => screen.getByRole('tab', { name });
const selectedTab = () => screen.getAllByRole('tab').find((t) => t.getAttribute('aria-selected') === 'true')?.textContent;

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(fakeContext() as never);
  HTMLCanvasElement.prototype.setPointerCapture = vi.fn();
  vi.spyOn(HTMLCanvasElement.prototype, 'getBoundingClientRect').mockReturnValue({ left: 0, top: 0, width: 320, height: 256, right: 320, bottom: 256, x: 0, y: 0, toJSON: () => ({}) });
});

describe('Composer, pannello a schede', () => {
  it('shows the four tabs, one panel at a time, starting from Foto', () => {
    composer();
    expect(screen.getAllByRole('tab').map((t) => t.textContent)).toEqual(['Foto', 'Ritocchi', 'Livelli', 'Modelli']);
    expect(selectedTab()).toBe('Foto');
    const panel = screen.getByRole('tabpanel');
    expect(within(panel).getByRole('button', { name: 'Scegli una foto…' })).toBeInTheDocument();
    expect(within(panel).queryByText('Zoom')).toBeNull();
    expect(within(panel).queryByRole('button', { name: '+ Testo' })).toBeNull();
  });

  it('changes the panel when a tab is clicked and remembers the choice in the browser', () => {
    const { unmount } = composer();
    fireEvent.click(tab('Ritocchi'));
    expect(selectedTab()).toBe('Ritocchi');
    expect(within(screen.getByRole('tabpanel')).getByText('Zoom')).toBeInTheDocument();
    expect(within(screen.getByRole('tabpanel')).queryByRole('button', { name: 'Scegli una foto…' })).toBeNull();
    fireEvent.click(tab('Modelli'));
    expect(within(screen.getByRole('tabpanel')).getByRole('button', { name: 'CQ' })).toBeInTheDocument();
    expect(localStorage.getItem('aurora.sstv.composerTab')).toBe('templates');
    unmount();
    composer();
    expect(selectedTab()).toBe('Modelli');
  });

  it('moves between the tabs with the arrow keys', () => {
    composer();
    fireEvent.keyDown(tab('Foto'), { key: 'ArrowRight' });
    expect(selectedTab()).toBe('Ritocchi');
    fireEvent.keyDown(tab('Ritocchi'), { key: 'ArrowLeft' });
    fireEvent.keyDown(tab('Foto'), { key: 'ArrowLeft' });
    expect(selectedTab()).toBe('Modelli');
  });

  it('shows the properties of a chosen layer inside the Livelli tab, below the list', () => {
    composer();
    fireEvent.click(tab('Livelli'));
    const panel = screen.getByRole('tabpanel');
    expect(within(panel).queryByText('Testo')).toBeNull();
    fireEvent.click(within(panel).getByRole('button', { name: '+ Testo' }));
    // il livello appena aggiunto e' scelto: le sue proprieta' compaiono nella stessa scheda
    expect(within(panel).getByRole('combobox')).toBeInTheDocument();
    expect(within(panel).getByRole('textbox')).toHaveValue('{MYCALL}');
  });

  it('jumps to Livelli when a layer is clicked on the preview', () => {
    composer();
    expect(selectedTab()).toBe('Foto');
    const overlay = screen.getByLabelText(/Un clic su un livello|Click a layer|livello/i, { selector: 'canvas' });
    fireEvent.pointerDown(overlay, { button: 0, clientX: 100, clientY: 100, pointerId: 1 });
    expect(selectedTab()).toBe('Livelli');
  });

  it('jumps to Foto when a photo is dropped on the preview', async () => {
    localStorage.setItem('aurora.sstv.composerTab', 'layers');
    const { container } = composer();
    expect(selectedTab()).toBe('Livelli');
    const board = container.querySelector('.sstv-artboard');
    expect(board).not.toBeNull();
    const file = new File(['x'], 'foto.png', { type: 'image/png' });
    await act(async () => {
      fireEvent.drop(board as Element, { dataTransfer: { files: [file], items: [] } });
    });
    await waitFor(() => expect(selectedTab()).toBe('Foto'));
  });
});
