import test from 'node:test';
import assert from 'node:assert/strict';
import { registerHooks } from 'node:module';
registerHooks({resolve(specifier,context,next){return next(specifier.endsWith('/stats/stats')?specifier+'.ts':specifier,context);}});
const { reportDates, overviewScope, percentage } = await import('../../src/lib/data/scope.ts');
test('report presets use local calendar dates including leap and year boundaries', () => {
  assert.deepEqual(reportDates('recent', new Date(2025, 0, 3)), {
    start: '2024-12-28',
    end: '2025-01-03',
  });
  assert.deepEqual(reportDates('previous', new Date(2024, 2, 15)), {
    start: '2024-02-01',
    end: '2024-02-29',
  });
  assert.deepEqual(reportDates('month', new Date(2024, 1, 15)), {
    start: '2024-02-01',
    end: '2024-02-29',
  });
  assert.deepEqual(reportDates('all'), { start: null, end: null });
});
test('today, week and selected year are inherited without lifetime substitution', () => {
  assert.equal(overviewScope('today', '2026-10-03', 2024).start, '2026-10-03');
  assert.equal(overviewScope('week', '2026-10-03', 2024).start, '2026-09-27');
  assert.deepEqual(overviewScope('alltime', '2026-10-03', 2024), {
    kind: 'daily',
    start: '2024-01-01',
    end: '2024-12-31',
    filter: { kind: 'all' },
    hour: null,
  });
});
test('time percentages use the total, handle tiny values, do not force sum to 100', () => {
  assert.deepEqual(
    [21600, 10800, 3600].map((v) => percentage(v, 36000)),
    ['60%', '30%', '10%']
  );
  assert.equal(percentage(1, 10000), '<0.1%');
  assert.equal(percentage(1, 1), '100%');
  assert.equal(percentage(0, 0), '0%');
  assert.equal(percentage(1, 3), '33.3%');
});
