import { act, fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SstvRxStatus } from '../../api/client';

vi.mock('../../api/client', () => ({ sstvTune: vi.fn(() => Promise.resolve()) }));
import { sstvTune } from '../../api/client';
import { emitSstv } from '../../lib/sstvBus';
import { SstvTuning, hzToLevel, levelToHz, stepsForSyncAt, tuneMarks } from './SstvTuning';

const RX: SstvRxStatus = { listening: true, receiving: true, mode: 'm1', width: 320, height: 256, rows: 20, sync: 1, offset_hz: 0, level_db: -30, last_image: null, tune_hz: 0, hist: [] };

beforeEach(() => {
  vi.mocked(sstvTune).mockClear();
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(null);
});

describe('SstvTuning', () => {
  it('maps levels to frequencies like the receiver: 1900 Hz is level 128, 6.25 Hz per level', () => {
    expect(levelToHz(128)).toBe(1900);
    expect(levelToHz(16)).toBe(1200);
    expect(hzToLevel(1500)).toBe(64);
    expect(hzToLevel(2300)).toBe(192);
    expect(tuneMarks(100).map((m) => m.hz)).toEqual([1300, 1600, 2400]);
    expect(stepsForSyncAt(1300)).toBe(16);
    expect(stepsForSyncAt(3000)).toBe(32);
  });

  it('moves in 6.25 Hz steps with the slider, the wheel and back to 0 with a double click', () => {
    render(<SstvTuning rx={RX} />);
    const slider = screen.getByRole('slider');
    expect(screen.getByText('Auto (AFC)')).toBeInTheDocument();
    fireEvent.change(slider, { target: { value: '16' } });
    expect(sstvTune).toHaveBeenLastCalledWith(16);
    expect(screen.getByText('+100 Hz')).toBeInTheDocument();
    fireEvent.wheel(screen.getByTestId('sstv-tuning'), { deltaY: -100 });
    expect(sstvTune).toHaveBeenLastCalledWith(17);
    expect(screen.getByText('+106.25 Hz')).toBeInTheDocument();
    fireEvent.doubleClick(slider);
    expect(sstvTune).toHaveBeenLastCalledWith(0);
  });

  it('says the tuning is right or how far to move the VFO from the AFC estimate', () => {
    const { rerender } = render(<SstvTuning rx={{ ...RX, offset_hz: 3 }} />);
    expect(screen.getByRole('status')).toHaveTextContent('Sintonia giusta');
    rerender(<SstvTuning rx={{ ...RX, offset_hz: -60 }} />);
    act(() => emitSstv({ type: 'sstv', kind: 'rx_status', offset_hz: -60 }));
    act(() => emitSstv({ type: 'sstv', kind: 'rx_hist', hist: new Array(256).fill(0) }));
    expect(screen.getByRole('status')).toHaveTextContent('Sposta il VFO di −60 Hz');
  });

  it('puts the sync where the histogram is clicked', () => {
    render(<SstvTuning rx={RX} />);
    const canvas = screen.getByRole('img', { name: 'Istogramma dei livelli del segnale' });
    vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({ left: 0, top: 0, width: 256, height: 34, right: 256, bottom: 34, x: 0, y: 0, toJSON: () => ({}) });
    // livello 32 = 1300 Hz: il sincronismo a +100 Hz
    fireEvent.click(canvas, { clientX: 32.5 });
    expect(sstvTune).toHaveBeenLastCalledWith(16);
  });
});
