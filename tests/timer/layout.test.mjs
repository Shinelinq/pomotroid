import test from 'node:test';
import assert from 'node:assert/strict';
import { fitMainTimer, timerMetrics } from '../../src/lib/utils/mainTimerLayout.ts';

test('baseline and upper limits describe the complete group', () => {
  assert.equal(timerMetrics(1).height, 378);
  const max = timerMetrics(1.4);
  assert.equal(max.play, 64);
  assert.ok(Math.abs(max.dial - 313.6) < 1e-6);
  assert.equal(max.controlGap, 16);
  assert.equal(max.border, 2);
  assert.equal(max.labelFont, 16);
  assert.equal(max.auxiliaryFont, 14);
  assert.ok(timerMetrics(0.85).height > 378 * 0.85, 'minimum hit areas are budgeted');
});

test('requested client sizes fit width and full height, with balanced outer space', () => {
  for (const [width, height] of [
    [345, 481],
    [517, 701],
    [760, 480],
    [345, 760],
    [900, 900],
  ]) {
    for (const rows of [0, 1, 2]) {
      const w = width - 32;
      const h = height - 40 - 28 - 32;
      const layout = fitMainTimer(w, h, rows, h + 28);
      assert.equal(layout.compact, false);
      assert.ok(layout.dial <= w * 0.86 + 1e-6);
      assert.ok(layout.height <= h + 1e-6);
      assert.ok(layout.scale >= 0.85 && layout.scale <= 1.4);
      if (layout.scale < 1.3999) {
        const next = timerMetrics(layout.scale + 0.001, rows);
        assert.ok(next.height > h || next.dial > w * 0.86, 'largest fitting scale');
      }
    }
  }
  const small = fitMainTimer(313, 381);
  assert.ok(small.dial >= 220 && small.dial <= 230);
  const large = fitMainTimer(485, 601);
  assert.ok(large.dial >= 300 && large.dial <= 314);
  assert.ok(large.time >= 64 && large.play >= 62);
  assert.ok(fitMainTimer(728, 380).scale < 1.01, 'short windows are height limited');
});

test('compact threshold budgets controls and extra arrangement rows', () => {
  assert.equal(fitMainTimer(208, 140, 0, 168).compact, true);
  assert.ok(fitMainTimer(208, 140, 0, 168).height <= 168);
  const h = timerMetrics(0.85).height;
  assert.equal(fitMainTimer(313, h).compact, false);
  assert.equal(fitMainTimer(313, h, 1, h + 28).compact, true);
  assert.equal(fitMainTimer(313, h + 20, 1).compact, false);
});
