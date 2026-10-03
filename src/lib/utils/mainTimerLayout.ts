// Main-window geometry in CSS pixels. MiniTimer deliberately does not use this model.
const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
export const MIN_TIMER_SCALE = 0.85;
export const MAX_TIMER_SCALE = 1.4;

export function timerMetrics(scale: number, extraStatusRows = 0) {
  const dial = 224 * scale;
  const labelHeight = clamp(20 * scale, 18, 24);
  const statusHeight = clamp(22 * scale, 20, 26);
  const footerHeight = clamp(26 * scale, 24, 30);
  const play = clamp(48 * scale, 44, 64);
  const labelGap = clamp(8 * scale, 6, 10);
  const statusGap = clamp(6 * scale, 4, 8);
  const controlGap = clamp(12 * scale, 8, 16);
  return {
    scale,
    dial,
    time: 48 * scale,
    play,
    playIcon: 22 * scale,
    side: Math.max(32, 36 * scale),
    sideIcon: 18 * scale,
    border: clamp(1.5 * scale, 1.5, 2),
    controls: 180 * scale,
    labelFont: clamp(14 * scale, 12, 16),
    auxiliaryFont: clamp(12 * scale, 11, 14),
    labelHeight,
    statusHeight,
    footerHeight,
    labelGap,
    statusGap,
    controlGap,
    height:
      dial +
      labelGap +
      labelHeight +
      statusGap +
      statusHeight * (1 + extraStatusRows) +
      controlGap +
      play +
      controlGap +
      footerHeight,
  };
}

// width/height are the stage content box, already excluding its padding and toolbar.
// compactHeight includes the space recovered by hiding the toolbar.
export function fitMainTimer(
  width: number,
  height: number,
  extraStatusRows = 0,
  compactHeight = height
) {
  const fits = (scale: number) => {
    const metrics = timerMetrics(scale, extraStatusRows);
    return metrics.dial <= width * 0.86 && metrics.height <= height;
  };
  if (width < 268 || !fits(MIN_TIMER_SCALE)) {
    const dial = Math.max(0, Math.min(224, width * 0.86, compactHeight - 40));
    return { ...timerMetrics(dial / 224), dial, height: dial + 40, compact: true };
  }
  let low = MIN_TIMER_SCALE;
  let high = MAX_TIMER_SCALE;
  // H(s) is monotone but piecewise because text and controls have size clamps.
  for (let i = 0; i < 24; i++) {
    const middle = (low + high) / 2;
    if (fits(middle)) low = middle;
    else high = middle;
  }
  return { ...timerMetrics(low, extraStatusRows), compact: false };
}

export type MainTimerLayout = ReturnType<typeof fitMainTimer>;

export function timerLayoutStyle(layout: MainTimerLayout) {
  return Object.entries(layout)
    .filter(([key]) => key !== 'compact')
    .map(([key, value]) => `--main-timer-${key}: ${value}${key === 'scale' ? '' : 'px'}`)
    .join(';');
}
