import { useSyncExternalStore } from 'react';
import { useT } from '../../i18n';
import { getPileup, onPileup, pileupTarget } from '../../lib/dxPileup';
import '../../styles/digi-radar.css';

/**
 * Modo DXpedition sulla cascata: la banda «DX ascolta qui» alla frequenza stimata (larga quanto la finestra degli ultimi chiamanti) e una
 * scia di trattini dove stavano gli ultimi chiamanti collegati, piu' sbiaditi i piu' vecchi.
 */
export function PileupMarks({ minHz, maxHz, top, xOf }: { minHz: number; maxHz: number; top: number; xOf(hz: number): number }) {
  const t = useT();
  const p = useSyncExternalStore(onPileup, getPileup);
  const target = pileupTarget(p);
  if (!p.dx) return null;
  const inside = (hz: number) => hz >= minHz && hz <= maxHz;
  const [lo, hi] = p.windowHz ?? [target ?? 0, target ?? 0];
  return (
    <>
      {p.worked.filter((w) => inside(w.hz)).map((w, i) => (
        <span key={`${w.call}-${w.at}`} className="digi-wf-pile-trail" style={{ left: xOf(w.hz), top, opacity: Math.max(0.25, 1 - i * 0.1) }} title={w.call} aria-hidden="true" />
      ))}
      {target !== null && inside(target) ? (
        <span
          className={`digi-wf-pile${p.estimateHz === null ? ' digi-wf-pile--hint' : ''}`}
          style={{ left: Math.min(xOf(lo), xOf(target)) - 4, width: Math.max(8, Math.abs(Math.max(xOf(hi), xOf(target)) - Math.min(xOf(lo), xOf(target))) + 8), top }}
          aria-hidden="true"
        >
          <span className="digi-wf-pile-label" style={{ left: xOf(target) - Math.min(xOf(lo), xOf(target)) + 4 }}>
            {t('digi.wf.pileup')}
          </span>
        </span>
      ) : null}
    </>
  );
}
