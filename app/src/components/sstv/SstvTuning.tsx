import { useEffect, useRef, useState } from 'react';
import { sstvTune, type SstvRxStatus } from '../../api/client';
import { useT } from '../../i18n';
import { onSstv } from '../../lib/sstvBus';

/** Un livello del segnale memorizzato vale 6,25 Hz; il livello 128 e' 1900 Hz (256 livelli: da 1100 a 2700 Hz). */
export const TUNE_STEP_HZ = 6.25;
export const TUNE_MAX_STEPS = 32;
const HIST_BINS = 256;
const HIST_CAP = 40;
/** Sotto questo scarto la sintonia si dice giusta (due passi: l'immagine non cambia a occhio). */
const GOOD_HZ = 12.5;

/** Frequenza al centro del livello `level`. */
export function levelToHz(level: number): number {
  return 1900 + (level - 128) * TUNE_STEP_HZ;
}

/** Livello (frazionario) di una frequenza. */
export function hzToLevel(hz: number): number {
  return 128 + (hz - 1900) / TUNE_STEP_HZ;
}

/** I tre segni: sincronismo (corto), nero e bianco (lunghi), spostati dalla correzione. */
export function tuneMarks(tuneHz: number): { hz: number; kind: 'sync' | 'black' | 'white' }[] {
  return [
    { hz: 1200 + tuneHz, kind: 'sync' },
    { hz: 1500 + tuneHz, kind: 'black' },
    { hz: 2300 + tuneHz, kind: 'white' },
  ];
}

/** I passi che portano il sincronismo sul punto `hz` cliccato. */
export function stepsForSyncAt(hz: number): number {
  return Math.max(-TUNE_MAX_STEPS, Math.min(TUNE_MAX_STEPS, Math.round((hz - 1200) / TUNE_STEP_HZ)));
}

/**
 * La sintonia SSTV in un riquadro: l'istogramma dei livelli del segnale degli ultimi secondi (blocco pieno), tre lineette (corta al
 * sincronismo, lunghe al nero e al bianco) spostate dalla correzione, il cursore della correzione a passi da 6,25 Hz e una riga che dice
 * se la sintonia e' giusta o di quanto girare il VFO. Con il segnale sulle lineette la sintonia e' giusta.
 */
export function SstvTuning({ rx, onChanged }: { rx: SstvRxStatus; onChanged?: () => void }) {
  const t = useT();
  const canvas = useRef<HTMLCanvasElement>(null);
  const [hist, setHist] = useState<number[]>(rx.hist ?? []);
  const [steps, setSteps] = useState(Math.round((rx.tune_hz ?? 0) / TUNE_STEP_HZ));
  const [offset, setOffset] = useState(rx.offset_hz);
  const pending = useRef(0);

  // lo stato del core (ogni 0,4 s mentre si ascolta) porta istogramma e correzione; un comando appena partito ha la precedenza
  useEffect(
    () =>
      onSstv((ev) => {
        if (ev.kind !== 'rx_status') return;
        if (ev.hist) setHist(ev.hist);
        if (typeof ev.offset_hz === 'number') setOffset(ev.offset_hz);
        if (typeof ev.tune_hz === 'number' && Date.now() - pending.current > 1500) setSteps(Math.round(ev.tune_hz / TUNE_STEP_HZ));
      }),
    [],
  );
  useEffect(() => {
    if (Date.now() - pending.current > 1500) setSteps(Math.round((rx.tune_hz ?? 0) / TUNE_STEP_HZ));
  }, [rx.tune_hz]);

  const tuneHz = steps * TUNE_STEP_HZ;

  const apply = (next: number) => {
    const s = Math.max(-TUNE_MAX_STEPS, Math.min(TUNE_MAX_STEPS, Math.round(next)));
    pending.current = Date.now();
    setSteps(s);
    void sstvTune(s).then(onChanged, () => undefined);
  };

  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const draw = () => {
      const ctx = c.getContext?.('2d');
      if (!ctx) return;
      const dpr = window.devicePixelRatio || 1;
      const rect = c.getBoundingClientRect();
      const w = Math.max(1, Math.round(rect.width * dpr));
      const h = Math.max(1, Math.round(rect.height * dpr));
      if (c.width !== w) c.width = w;
      if (c.height !== h) c.height = h;
      const css = getComputedStyle(c);
      const ink = css.getPropertyValue('--sstv-hist-ink').trim() || '#000';
      const mark = css.getPropertyValue('--sstv-hist-mark').trim() || '#d00';
      ctx.clearRect(0, 0, w, h);
      const binW = w / HIST_BINS;
      ctx.fillStyle = ink;
      const top = 7 * dpr;
      for (let i = 0; i < HIST_BINS; i += 1) {
        const v = Math.min(HIST_CAP, hist[i] ?? 0);
        if (v <= 0) continue;
        const bh = (v / HIST_CAP) * (h - top);
        ctx.fillRect(i * binW, h - bh, Math.ceil(binW), bh);
      }
      ctx.fillStyle = mark;
      for (const m of tuneMarks(tuneHz)) {
        const x = Math.round(((hzToLevel(m.hz) + 0.5) / HIST_BINS) * w);
        const len = m.kind === 'sync' ? h * 0.45 : h;
        ctx.fillRect(x - Math.max(1, dpr), 0, Math.max(2, 2 * dpr), len);
      }
    };
    draw();
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(draw);
    observer?.observe(c);
    return () => observer?.disconnect();
  }, [hist, tuneHz]);

  // la stima dell'AFC (o la correzione a mano) dice di quanto spostare il VFO; in USB alzare il VFO abbassa il segnale nell'audio
  const shift = steps !== 0 ? tuneHz : rx.receiving || rx.rows > 0 ? offset : null;
  const hint = shift === null ? t('sstv.tune.idle') : Math.abs(shift) < GOOD_HZ ? t('sstv.tune.good') : t('sstv.tune.move', { hz: `${shift > 0 ? '+' : '−'}${Math.round(Math.abs(shift))}` });
  const value = steps === 0 ? t('sstv.tune.auto') : `${tuneHz > 0 ? '+' : '−'}${Math.abs(tuneHz).toFixed(tuneHz % 1 === 0 ? 0 : 2)} Hz`;

  return (
    <div
      className="sstv-tune"
      data-testid="sstv-tuning"
      onWheel={(e) => {
        e.preventDefault();
        apply(steps + (e.deltaY < 0 ? 1 : -1));
      }}
    >
      <canvas
        ref={canvas}
        className="sstv-tune-hist"
        role="img"
        aria-label={t('sstv.tune.hist')}
        title={t('sstv.tune.histHint')}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          if (r.width <= 0) return;
          apply(stepsForSyncAt(levelToHz(((e.clientX - r.left) / r.width) * HIST_BINS - 0.5)));
        }}
      />
      <div className="sstv-tune-row">
        <label className="sstv-tune-slider" title={t('sstv.tune.sliderHint')}>
          <span>{t('sstv.tune.label')}</span>
          <input
            type="range"
            min={-TUNE_MAX_STEPS}
            max={TUNE_MAX_STEPS}
            step={1}
            value={steps}
            aria-valuetext={value}
            onChange={(e) => apply(Number(e.target.value))}
            onDoubleClick={() => apply(0)}
          />
        </label>
        <b className="sstv-tune-value" onDoubleClick={() => apply(0)}>
          {value}
        </b>
        <span className={`sstv-tune-hint${shift !== null && Math.abs(shift) < GOOD_HZ ? ' sstv-tune-hint--ok' : ''}`} role="status">
          {hint}
        </span>
      </div>
    </div>
  );
}
