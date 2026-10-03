import test from 'node:test';
import assert from 'node:assert/strict';
import {
  formatDuration,
  formatRate,
  buildWeek,
  summarizeWeek,
  summarizeYear,
  buildYear,
  heatLevel,
  moveDate,
  dateKey,
  shiftDate,
  chartScale,
  hourRange,
} from '../../src/lib/components/stats/stats.ts';

test('duration keeps whole minutes and cumulative hours', () => {
  for (const [seconds, expected] of [
    [0, '0m'],
    [59, '0m'],
    [2700, '45m'],
    [3600, '1h'],
    [4200, '1h 10m'],
    [6300, '1h 45m'],
    [1123500, '312h 5m'],
    [3637, '1h'],
    [4294967418, '1193046h 30m'],
  ]) {
    assert.equal(formatDuration(seconds), expected);
  }
});

test('week uses weighted completion and active-day average, including incomplete-only days', () => {
  const days = buildWeek(
    [
      { date: '2024-03-15', rounds: 2, started_rounds: 3, focus_secs: 4200 },
      { date: '2024-03-14', rounds: 0, started_rounds: 5, focus_secs: 0 },
      { date: '2024-03-13', rounds: 1, started_rounds: 1, focus_secs: 900 },
      { date: '2024-03-08', rounds: 20, started_rounds: 20, focus_secs: 30000 },
      { date: '2024-03-16', rounds: 20, started_rounds: 20, focus_secs: 30000 },
    ],
    '2024-03-15'
  );
  assert.equal(days.length, 7);
  assert.deepEqual(days[0], { date: '2024-03-09', rounds: 0, started_rounds: 0, focus_secs: 0 });
  assert.equal(days[5].started_rounds, 5);
  assert.deepEqual(summarizeWeek(days), {
    rounds: 3,
    started: 9,
    seconds: 5100,
    active: 2,
    average: 2550,
  });
  assert.equal(formatRate(3, 9), '33%');
  assert.equal(formatRate(0, 0), '—');
  assert.equal(formatRate(0, 2), '0%');
  assert.equal(formatDuration(summarizeWeek(buildWeek([], '2024-03-15')).average), '0m');
});

test('local calendar arithmetic spans years, leap days and DST dates', () => {
  assert.equal(dateKey(new Date(2024, 1, 29)), '2024-02-29');
  assert.equal(shiftDate('2024-03-01', -1), '2024-02-29');
  assert.equal(shiftDate('2024-03-10', 1), '2024-03-11');
  assert.deepEqual(
    buildWeek([], '2025-01-03').map((d) => d.date),
    [
      '2024-12-28',
      '2024-12-29',
      '2024-12-30',
      '2024-12-31',
      '2025-01-01',
      '2025-01-02',
      '2025-01-03',
    ]
  );
});

test('year summary changes by year; best-day ties choose earliest without mutating history', () => {
  const entries = [
    { date: '2024-12-31', count: 3, focus_secs: 6300 },
    { date: '2025-01-01', count: 2, focus_secs: 4200 },
    { date: '2024-02-29', count: 2, focus_secs: 6300 },
  ];
  const original = structuredClone(entries);
  assert.deepEqual(summarizeYear(entries, 2024), {
    seconds: 12600,
    rounds: 5,
    active: 2,
    best: entries[2],
  });
  assert.equal(summarizeYear(entries, 2025).seconds, 4200);
  assert.deepEqual(summarizeYear(entries, 2023), { seconds: 0, rounds: 0, active: 0, best: null });
  assert.deepEqual(entries, original);
});

test('heat levels have stable exact time and count boundaries', () => {
  for (const [value, expected] of [
    [0, 0],
    [1, 1],
    [3600, 1],
    [3601, 2],
    [10800, 2],
    [10801, 3],
  ])
    assert.equal(heatLevel(value, 'time'), expected);
  for (const [value, expected] of [
    [0, 0],
    [1, 1],
    [3, 1],
    [4, 2],
    [7, 2],
    [8, 3],
  ])
    assert.equal(heatLevel(value, 'rounds'), expected);
});

test('calendar retains all 54 weeks when required, leap day and selectable empty days', () => {
  const leap = buildYear([], 2000, '2026-10-02');
  assert.equal(leap.weeks, 54);
  assert.equal(leap.cells.length, 378);
  assert.equal(leap.cells.filter((cell) => cell.valid).length, 366);
  assert.equal(leap.months.length, 12);
  assert.deepEqual(
    leap.cells.find((cell) => cell.date === '2000-02-29'),
    { date: '2000-02-29', count: 0, focus_secs: 0, week: 9, weekday: 2, valid: true }
  );
  const current = buildYear([], 2024, '2024-03-15');
  assert.equal(current.cells.find((d) => d.date === '2024-03-15').valid, true);
  assert.equal(current.cells.find((d) => d.date === '2024-03-16').valid, false);
  assert.equal(current.cells.find((d) => d.date === '2023-12-31').valid, false);
});

test('keyboard navigation never enters future or padding dates', () => {
  const dates = new Set(
    buildYear([], 2024, '2024-03-15')
      .cells.filter((d) => d.valid)
      .map((d) => d.date)
  );
  assert.equal(moveDate('2024-03-15', 1, dates), '2024-03-15');
  assert.equal(moveDate('2024-01-01', -1, dates), '2024-01-01');
  assert.equal(moveDate('2024-03-15', -7, dates), '2024-03-08');
  assert.equal(moveDate('2024-02-29', 1, dates), '2024-03-01');
});

test('axes cover data using natural integer or minute intervals', () => {
  for (const metric of ['time', 'rounds'])
    for (const max of [0, 1, 5, 59, 4200, 1123500]) {
      const scale = chartScale(max, metric);
      assert.ok(scale.maximum >= max);
      assert.ok(scale.ticks.length >= 2 && scale.ticks.length <= 5);
      for (const tick of scale.ticks) assert.equal(tick % (metric === 'time' ? 60 : 1), 0);
    }
});

test('fifteen hours uses four-hour intervals with a sixteen-hour ceiling', () => {
  for (const height of [140, 240, 440]) {
    const scale = chartScale(15 * 3600, 'time', height);
    assert.deepEqual(scale, { maximum: 16 * 3600, ticks: [4, 8, 12, 16].map((h) => h * 3600) });
  }
});

test('hour boundaries end at 24:00 without creating a twenty-fifth bucket', () => {
  const buckets = Array.from({ length: 24 }, (_, h) => hourRange(h));
  assert.deepEqual(buckets[8], { start: '08:00', end: '09:00' });
  assert.deepEqual(buckets[23], { start: '23:00', end: '24:00' });
  assert.equal(buckets.length, 24);
});
