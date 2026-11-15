import { powerPct } from '../../lib/radioDisplay';
import { useT } from '../../i18n';
import { useCatExtras } from './RadioExtras';
import { SplitButton } from './SplitButton';

/**
 * Il riquadro PWR + SPLIT, uguale in ogni stile del display e accanto ad esso: dall'alto la scritta PWR, il valore (100%, «—» se la radio
 * non lo da'), i tasti − e + affiancati e SPLIT largo come il riquadro (verde pieno acceso, scuro spento; un clic porta la radio in simplex).
 * Piccolo: non allunga ne' il display ne' la testata. Le misure stanno in `.rd-pwrsplit` (radiodisplay.css).
 */
export function PwrSplitBox() {
  const t = useT();
  const { rig, waiting, click } = useCatExtras();
  const power = rig?.extras?.power_set ?? null;
  const locked = power === null || waiting !== null;
  return (
    <div className="rd-pwrsplit" role="group" aria-label="PWR">
      <span className="rd-pwrsplit-lbl">PWR</span>
      <span className="rd-pwrsplit-val data">{power === null ? '—' : `${powerPct(power)}%`}</span>
      <div className="rd-pwrsplit-btns">
        <button type="button" className="rd-pwrsplit-btn" title={t('rx.act.pwrDown')} aria-label={t('rx.act.pwrDown')} disabled={locked} onClick={() => void click('pwr_down')}>
          −
        </button>
        <button type="button" className="rd-pwrsplit-btn" title={t('rx.act.pwrUp')} aria-label={t('rx.act.pwrUp')} disabled={locked} onClick={() => void click('pwr_up')}>
          +
        </button>
      </div>
      <SplitButton split={rig?.split} placeholder />
    </div>
  );
}
