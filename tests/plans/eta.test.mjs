import test from 'node:test';
import assert from 'node:assert/strict';
import { eta } from '../../src/lib/plans/eta.ts';

const snapshot = (date, patch = {}) => ({
  has_started: true,
  is_running: true,
  total_secs: 1500,
  elapsed_secs: 300,
  captured_at_ms: date.getTime(),
  ...patch,
});
test('waiting and pause never show an obsolete finish time', () => {
  assert.deepEqual(eta(snapshot(new Date(), { has_started: false }), 'en'), { kind: 'waiting' });
  assert.deepEqual(eta(snapshot(new Date(), { is_running: false, elapsed_secs: 378 }), 'en'), {
    kind: 'paused',
    minutes: 18,
    seconds: 42,
  });
});
test('running and resumed ETA use the supplied snapshot time', () => {
  assert.equal(eta(snapshot(new Date(2026, 9, 3, 15, 20)), 'zh').time, '15:40');
  assert.equal(eta(snapshot(new Date(2026, 9, 3, 16, 20)), 'zh').time, '16:40');
});
test('local midnight and later dates are distinct', () => {
  const tomorrow = eta(snapshot(new Date(2026, 9, 3, 23, 59), { total_secs: 1560 }), 'zh');
  assert.equal(tomorrow.days, 1);
  assert.equal(tomorrow.time, '00:20');
  assert.equal(eta(snapshot(new Date(2026, 9, 3, 23, 59), { total_secs: 90000 }), 'en').days, 2);
});
test('remaining time clamps at zero without advancing a local clock', () => {
  const state = snapshot(new Date(2026, 9, 3, 15, 20), { elapsed_secs: 1600 });
  assert.equal(eta(state, 'zh').time, '15:20');
});
