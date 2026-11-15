import { useEffect, useRef } from 'react';
import { useT } from '../../i18n';
import { FrequencyRuler } from '../FrequencyRuler';
import { leftPercent } from '../../lib/markers';
import { registerSpectrum } from '../../lib/spectrumFeed';
import { ROW_SECS, useWfPrefs } from '../../lib/wfPrefs';
import { RowPacer, liftCurve, processRow, spectrumCurve } from '../../render/jtdxRow';
import { drawSpectrum } from '../../render/spectrumPlot';

/** Il tratto audio mostrato (Hz): VIS a 1100/1300, sincronismo a 1200, immagine da 1500 a 2300. */
export const SSTV_SPEC_MIN_HZ = 300;
export const SSTV_SPEC_MAX_HZ = 2800;
/** I segni fissi di sintonia, come nei programmi SSTV: VIS (1100/1300), sincronismo (1200), nero (1500), leader (1900), bianco (2300). */
export const SSTV_MARKS: readonly { hz: number; label: string; kind: 'sync' | 'vis' | 'level' }[] = [
  { hz: 1100, label: '1100', kind: 'vis' },
  { hz: 1200, label: '1200', kind: 'sync' },
  { hz: 1300, label: '1300', kind: 'vis' },
  { hz: 1500, label: '1500', kind: 'level' },
  { hz: 1900, label: '1900', kind: 'vis' },
  { hz: 2300, label: '2300', kind: 'level' },
];

/**
 * Lo spettro sottile sotto il visore SSTV: la stessa curva verde su griglia a puntini dello spettro della Stazione (`drawSpectrum`,
 * righe del core passate da `processRow` come nella cascata), i segni fissi e il righello delle frequenze. Niente barra «Rx» ne' stima
 * dello scarto: la sintonia si guarda e si corregge nel riquadro sotto (`SstvTuning`, istogramma dei livelli e cursore). Un clic su un
 * picco dello spettro (`onPick`) porta li' il sincronismo.
 */
export function SstvSpectrum({ height = 46, onPick }: { height?: number; /** Clic sullo spettro: la frequenza audio cliccata (Hz). */ onPick?: (hz: number) => void }) {
  const t = useT();
  const plot = useRef<HTMLCanvasElement>(null);
  const curve = useRef<Float32Array | null>(null);

  useEffect(() => {
    const canvas = plot.current;
    if (!canvas) return;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const w = Math.max(1, Math.round(rect.width * dpr));
      const h = Math.max(1, Math.round(rect.height * dpr));
      if (canvas.width !== w) canvas.width = w;
      if (canvas.height !== h) canvas.height = h;
      drawSpectrum(canvas, curve.current, SSTV_SPEC_MIN_HZ, SSTV_SPEC_MAX_HZ);
    };
    draw();
    // nell'ambiente delle prove non c'e'
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(draw);
    observer?.observe(canvas);
    const pacer = new RowPacer();
    const unregister = registerSpectrum((bins, fMinHz, fMaxHz) => {
      const p = useWfPrefs.getState().prefs;
      const average = pacer.add(bins, ROW_SECS[p.speed], p.navg);
      if (!average) return;
      const binWidthHz = (fMaxHz - fMinHz) / Math.max(1, bins.length);
      const row = processRow(average, binWidthHz, { binsPerPixel: p.binsPerPixel, startHz: SSTV_SPEC_MIN_HZ, endHz: SSTV_SPEC_MAX_HZ, scale: p.scale, flatten: p.flatten });
      if (row.y.length < 2) return;
      curve.current = liftCurve(spectrumCurve(row.y, 'current', p.gain2d, p.zero2d, p.flatten));
      draw();
    });
    return () => {
      unregister();
      observer?.disconnect();
    };
  }, []);

  return (
    <div className="sstv-spec" data-testid="sstv-spectrum">
      <div
        className="sstv-spec-plot"
        style={{ height }}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          if (!onPick || r.width <= 0) return;
          onPick(SSTV_SPEC_MIN_HZ + ((e.clientX - r.left) / r.width) * (SSTV_SPEC_MAX_HZ - SSTV_SPEC_MIN_HZ));
        }}
      >
        <canvas ref={plot} className="sstv-spec-canvas" aria-hidden="true" />
        {/* i segni fissi: il sincronismo del segnale deve cadere sulla riga a 1200, il nero a 1500, il bianco a 2300 */}
        {SSTV_MARKS.map((m) => (
          <span key={m.hz} className={`sstv-mark sstv-mark--${m.kind}`} style={{ left: `${leftPercent(m.hz, SSTV_SPEC_MIN_HZ, SSTV_SPEC_MAX_HZ)}%` }} title={t(`sstv.mark.${m.kind}`, { hz: m.hz })} aria-hidden="true">
            {m.kind === 'vis' ? null : <i>{m.label}</i>}
          </span>
        ))}
      </div>
      <FrequencyRuler fMinHz={SSTV_SPEC_MIN_HZ} fMaxHz={SSTV_SPEC_MAX_HZ} decodes={[]} />
    </div>
  );
}
