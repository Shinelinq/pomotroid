import test from 'node:test';
import assert from 'node:assert/strict';
import {
  exactDuration,
  sessionStatus,
  sessionTimes,
  shiftDetailDate,
} from '../../src/lib/components/stats/sessionDetails.ts';
import { createSessionList } from '../../src/lib/components/stats/sessionList.ts';

const scope = { date: '2026-01-01', hour: null, filter: { kind: 'all' } };
const record = (id, patch = {}) => ({
  id,
  started_at: 1767225600,
  ended_at: null,
  duration_secs: 5400,
  completed: false,
  category_id: null,
  category_name: null,
  category_archived: false,
  ...patch,
});
const runtime = {
  session_id: 3,
  round_type: 'work',
  has_started: true,
  is_running: true,
  is_paused: false,
};
function page(start, count, total = count, more = false) {
  return {
    records: Array.from({ length: count }, (_, i) => record(start + i)),
    summary: { recorded: total, completed: 1, focus_secs: 1801 },
    next_cursor: more
      ? { ...scope, started_at: 1767225600, id: start + count - 1, max_id: total }
      : null,
  };
}

test('only the explicitly identified work session is running or paused; old null ends stay incomplete', () => {
  assert.equal(sessionStatus(record(1), runtime), 'incomplete');
  assert.equal(sessionStatus(record(3), runtime), 'running');
  assert.equal(
    sessionStatus(record(3), { ...runtime, is_running: false, is_paused: true }),
    'paused'
  );
  assert.equal(sessionStatus(record(3, { completed: true }), runtime), 'completed');
  assert.equal(sessionStatus(record(3), { ...runtime, session_id: null }), 'incomplete');
  assert.equal(sessionStatus(record(3), { ...runtime, round_type: 'short-break' }), 'incomplete');
  assert.equal(sessionStatus(record(3), null), 'incomplete');
});

test('exact durations preserve seconds and end time is never inferred from planned duration', () => {
  assert.equal(exactDuration(5400), '1h 30m');
  assert.equal(exactDuration(339), '5m 39s');
  assert.equal(exactDuration(59), '59s');
  assert.equal(sessionTimes(record(1), 'en').end, null);
  const start = new Date(2025, 11, 31, 23, 50).getTime() / 1000;
  const end = new Date(2026, 0, 1, 0, 25).getTime() / 1000;
  const result = sessionTimes(record(1, { started_at: start, ended_at: end }), 'en');
  assert.equal(result.nextDay, true);
  assert.equal(result.end, '00:25');
  const later = sessionTimes(
    record(1, { started_at: start, ended_at: new Date(2026, 0, 3, 8, 0).getTime() / 1000 }),
    'en'
  );
  assert.equal(later.nextDay, false);
  assert.match(later.endDate, /2026/);
});

test('calendar navigation handles leap days, years and the supported lower boundary', () => {
  assert.equal(shiftDetailDate('2024-03-01', -1), '2024-02-29');
  assert.equal(shiftDetailDate('2025-12-31', 1), '2026-01-01');
  assert.equal(shiftDetailDate('0001-01-01', -1), null);
  assert.equal(shiftDetailDate('0099-12-31', 1), '0100-01-01');
});

test('changing date/hour immediately clears old rows and rejects late responses', async () => {
  const deferred = Promise.withResolvers();
  let state;
  const queries = [];
  const model = createSessionList(
    (query) => {
      queries.push(query);
      return query.date === scope.date ? deferred.promise : Promise.resolve(page(90, 1));
    },
    (next) => {
      state = next;
    }
  );
  const old = model.open(scope);
  const fresh = model.open({ ...scope, date: '2026-01-02', hour: 8 });
  assert.equal(state.records.length, 0);
  assert.equal(state.summary, null);
  await fresh;
  deferred.resolve(page(1, 3));
  await old;
  assert.equal(state.records[0].id, 90);
  assert.equal(queries[1].cursor, null);
  assert.equal(queries[1].hour, 8);
  model.dispose();
});

test('summary covers the range, load-more is bounded, refresh keeps the loaded pages', async () => {
  const calls = [];
  let state;
  const model = createSessionList(
    async (q) => {
      calls.push(q);
      const start = (q.cursor?.id ?? 0) + 1;
      return page(start, Math.min(50, 123 - start + 1), 123, start + 49 < 123);
    },
    (value) => {
      state = value;
    }
  );
  await model.open(scope);
  assert.equal(state.records.length, 50);
  assert.equal(state.summary.recorded, 123);
  await model.more();
  assert.equal(state.records.length, 100);
  await model.refresh();
  assert.equal(state.records.length, 100);
  assert.equal(state.records[99].id, 100);
  await model.more();
  assert.equal(state.records.length, 123);
  assert.equal(state.cursor, null);
  assert.ok(calls.every((q) => q.limit === 50));
  assert.equal(new Set(state.records.map((r) => r.id)).size, 123);
  model.dispose();
});

test('a live refresh during pagination retains the requested extra page and ignores the stale append', async () => {
  let calls = 0;
  let state;
  const pending = Promise.withResolvers();
  const model = createSessionList(
    async (q) => {
      ++calls;
      if (calls === 2) return pending.promise;
      return q.cursor ? page(51, 50, 120, true) : page(1, 50, 120, true);
    },
    (value) => {
      state = value;
    }
  );
  await model.open(scope);
  const more = model.more();
  await model.refresh();
  assert.equal(state.records.length, 100);
  pending.resolve(page(51, 50, 119, true));
  await more;
  assert.equal(state.records.length, 100);
  assert.equal(state.summary.recorded, 120);
  model.dispose();
});

test('errors remain distinct from true empty data and retry can recover', async () => {
  let failing = true;
  let state;
  const model = createSessionList(
    async () => {
      if (failing) throw new Error('database unavailable');
      return page(1, 0, 0);
    },
    (value) => {
      state = value;
    }
  );
  await model.open(scope);
  assert.equal(state.summary, null);
  assert.match(state.error.message, /database unavailable/);
  failing = false;
  await model.retry();
  assert.equal(state.error, null);
  assert.equal(state.summary.recorded, 0);
  model.dispose();
});

test('destroyed detail views cannot be overwritten by late requests', async () => {
  let updates = 0;
  const pending = Promise.withResolvers();
  const model = createSessionList(
    () => pending.promise,
    () => updates++
  );
  const load = model.open(scope);
  model.dispose();
  const before = updates;
  pending.resolve(page(1, 2));
  await load;
  assert.equal(updates, before);
});

test('an invalidated cursor retries the range instead of repeatedly using the stale cursor', async () => {
  let count = 0;
  const queries = [];
  let state;
  const model = createSessionList(
    async (query) => {
      queries.push(query);
      if (++count === 1) return page(1, 50, 100, true);
      if (count === 2) throw 'detail_invalid_cursor';
      return page(101, 3, 3);
    },
    (value) => {
      state = value;
    }
  );
  await model.open(scope);
  await model.more();
  assert.equal(state.error.operation, 'more');
  await model.retry();
  assert.equal(queries[2].cursor, null);
  assert.equal(state.records.length, 3);
  assert.equal(state.summary.recorded, 3);
  assert.equal(state.error, null);
  model.dispose();
});
