import test from 'node:test';
import assert from 'node:assert/strict';
import { syncMiniTimer, miniDisplay } from '../../src/lib/mini/sync.ts';

const base = {
  round_type: 'work',
  previous_round_type: '',
  elapsed_secs: 300,
  total_secs: 1500,
  is_running: true,
  is_paused: false,
  work_round_number: 2,
  work_rounds_total: 4,
  session_work_count: 2,
};
const names = ['onTimerTick', 'onTimerPaused', 'onTimerResumed', 'onRoundChange', 'onTimerReset'];
const turn = () => new Promise((resolve) => setImmediate(resolve));
function fixture(read = async () => ({ ...base })) {
  const listeners = {},
    removed = [],
    registered = [];
  const api = { getTimerState: read };
  for (const name of names)
    api[name] = async (callback) => {
      listeners[name] = callback;
      registered.push(name);
      return () => removed.push(name);
    };
  return { api, listeners, removed, registered };
}

test('display uses remaining seconds, clamped progress and full minute digits', () => {
  assert.deepEqual(miniDisplay(base), { remaining: 1200, ratio: 0.8, text: '20:00', fontSize: 24 });
  assert.equal(miniDisplay({ ...base, total_secs: 6005, elapsed_secs: 0 }).text, '100:05');
  assert.equal(miniDisplay({ ...base, total_secs: 6005, elapsed_secs: 0 }).fontSize, 22);
  for (const total_secs of [0, 200]) {
    const result = miniDisplay({ ...base, total_secs });
    assert.equal(result.text, '00:00');
    assert.equal(result.ratio, 0);
  }
});

test('all listeners precede the initial snapshot, which is displayed without an initial tween', async () => {
  const updates = [];
  const f = fixture(async () => {
    assert.equal(f.registered.length, 5);
    return base;
  });
  const sync = syncMiniTimer(f.api, (...args) => updates.push(args));
  await sync.ready;
  assert.deepEqual(updates, [[base, false]]);
  sync.dispose();
  sync.dispose();
  assert.deepEqual(f.removed.sort(), [...names].sort());
});

test('late snapshot never overwrites a newer round event', async () => {
  const pending = Promise.withResolvers();
  const next = { ...base, round_type: 'short-break', elapsed_secs: 0, total_secs: 300 };
  let reads = 0;
  const f = fixture(() => (++reads === 1 ? pending.promise : Promise.resolve(next)));
  const updates = [];
  const sync = syncMiniTimer(f.api, (value) => updates.push(value));
  await turn();
  f.listeners.onRoundChange(next);
  pending.resolve(base);
  await sync.ready;
  assert.equal(reads, 2);
  assert.ok(updates.every((value) => value.round_type === 'short-break'));
  sync.dispose();
});

test('only a normal tick smooths; pause, resume, skip, restart and corrections snap', async () => {
  const f = fixture();
  const updates = [];
  const sync = syncMiniTimer(f.api, (value, smooth) => updates.push({ value, smooth }));
  await sync.ready;
  f.listeners.onTimerTick({ elapsed_secs: 301, total_secs: 1500 });
  assert.equal(updates.at(-1).smooth, true);
  f.listeners.onTimerPaused({ elapsed_secs: 301 });
  assert.equal(updates.at(-1).value.is_paused, true);
  assert.equal(updates.at(-1).smooth, false);
  f.listeners.onTimerResumed({ elapsed_secs: 301 });
  assert.equal(updates.at(-1).value.is_running, true);
  f.listeners.onTimerTick({ elapsed_secs: 900, total_secs: 1500 });
  assert.equal(updates.at(-1).smooth, false);
  f.listeners.onTimerReset({ ...base, elapsed_secs: 0 });
  assert.equal(updates.at(-1).smooth, false);
  f.listeners.onRoundChange({
    ...base,
    round_type: 'long-break',
    elapsed_secs: 0,
    total_secs: 900,
  });
  assert.equal(updates.at(-1).value.round_type, 'long-break');
  assert.equal(updates.at(-1).smooth, false);
  sync.dispose();
});

test('destroy during registration releases every late listener and never reads the timer', async () => {
  const pending = names.map(() => Promise.withResolvers());
  const removed = [];
  let reads = 0;
  const api = {
    getTimerState: async () => {
      ++reads;
      return base;
    },
  };
  names.forEach((name, i) => {
    api[name] = () => pending[i].promise;
  });
  const sync = syncMiniTimer(api, () => assert.fail('destroyed view updated'));
  sync.dispose();
  pending.forEach((item, i) => item.resolve(() => removed.push(i)));
  await sync.ready;
  assert.equal(reads, 0);
  assert.equal(removed.length, 5);
});

test('destroy during snapshot read cannot update an unmounted view', async () => {
  const pending = Promise.withResolvers();
  const f = fixture(() => pending.promise);
  const sync = syncMiniTimer(f.api, () => assert.fail('late snapshot applied'));
  await turn();
  sync.dispose();
  pending.resolve(base);
  await sync.ready;
  assert.equal(f.removed.length, 5);
});

test('failed registration releases successful and late registrations', async () => {
  const late = Promise.withResolvers();
  const f = fixture();
  f.api.onTimerTick = () => Promise.reject(new Error('listen failed'));
  f.api.onTimerReset = () => late.promise;
  const sync = syncMiniTimer(f.api, () => assert.fail('failed setup updated the view'));
  await assert.rejects(sync.ready, /listen failed/);
  late.resolve(() => f.removed.push('onTimerReset'));
  await turn();
  assert.equal(f.removed.length, 4);
});

test('snapshot errors are reported rather than synthesizing countdown state', async () => {
  const f = fixture(async () => {
    throw new Error('snapshot failed');
  });
  const sync = syncMiniTimer(f.api, () => assert.fail('fake state emitted'));
  await assert.rejects(sync.ready, /snapshot failed/);
  assert.equal(f.removed.length, 5);
});

test('full backend snapshots synchronize stopped, pending and zero-second paused rounds without a frontend clock', async () => {
  const listeners = {};
  let resolveRead;
  const initial = new Promise((resolve) => {
    resolveRead = resolve;
  });
  const updates = [];
  let reads = 0;
  const full = { ...base, round_id: 7, revision: 1, has_started: true, stop_after_round: false };
  const api = {
    getTimerState: () =>
      ++reads === 1
        ? initial
        : Promise.resolve({
            ...full,
            revision: 3,
            is_running: false,
            is_paused: true,
            elapsed_secs: 0,
            stop_after_round: true,
          }),
    onTimerState: async (callback) => {
      listeners.state = callback;
      return () => {};
    },
  };
  const sync = syncMiniTimer(api, (state) => updates.push(state));
  await turn();
  listeners.state({
    ...full,
    revision: 3,
    is_running: false,
    is_paused: true,
    elapsed_secs: 0,
    stop_after_round: true,
  });
  resolveRead(full);
  await sync.ready;
  assert.equal(updates.at(-1).revision, 3);
  assert.equal(updates.at(-1).stop_after_round, true);
  assert.equal(updates.at(-1).is_paused, true);
  listeners.state({
    ...full,
    round_id: 8,
    revision: 4,
    stop_after_round: false,
    has_started: false,
    is_running: false,
  });
  assert.equal(updates.at(-1).round_id, 8);
  assert.equal(updates.at(-1).has_started, false);
  listeners.state(full);
  assert.equal(updates.at(-1).round_id, 8);
  sync.dispose();
});

test('main and mini consume the same locked/next category snapshot without starting another timer', async () => {
  let receive;
  const main = {
    ...base,
    revision: 20,
    round_id: 9,
    category_id: 1,
    next_category_id: 2,
    category_pending: true,
    category_notice_id: null,
  };
  let view;
  const sync = syncMiniTimer(
    {
      getTimerState: async () => main,
      onTimerState: async (callback) => {
        receive = callback;
        return () => {};
      },
    },
    (state) => {
      view = state;
    }
  );
  await sync.ready;
  assert.deepEqual(view, main);
  receive({ ...main, revision: 21, next_category_id: null, category_notice_id: 2 });
  assert.equal(view.category_id, 1);
  assert.equal(view.next_category_id, null);
  assert.equal(miniDisplay(view).remaining, miniDisplay(main).remaining);
  receive({
    ...main,
    revision: 22,
    round_id: 10,
    round_type: 'short-break',
    category_id: null,
    next_category_id: null,
    category_pending: false,
  });
  assert.equal(view.category_id, null);
  assert.equal(view.round_id, 10);
  sync.dispose();
});
