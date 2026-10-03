import test from 'node:test';
import assert from 'node:assert/strict';
import { createRequestScope } from '../../src/lib/components/stats/requestScope.ts';

function fixture() {
  const requests = createRequestScope();
  const values = { detailed: null, heatmap: null };
  const loading = { detailed: false, heatmap: false };
  const errors = { detailed: null, heatmap: null };
  async function query(channel, promise) {
    const current = requests.begin(channel);
    loading[channel] = true;
    try {
      const result = await promise;
      if (current()) values[channel] = result;
    } catch (error) {
      if (current()) errors[channel] = error;
    } finally {
      if (current()) loading[channel] = false;
    }
  }
  return { requests, values, loading, errors, query };
}

test('rapid all → uncategorized → category switches cannot show a late old result', async () => {
  const f = fixture();
  const all = Promise.withResolvers(),
    uncategorized = Promise.withResolvers(),
    category = Promise.withResolvers();
  const a = f.query('detailed', all.promise);
  f.requests.invalidate();
  const u = f.query('detailed', uncategorized.promise);
  f.requests.invalidate();
  const c = f.query('detailed', category.promise);
  category.resolve({ scope: 'category:7', rounds: 2 });
  await c;
  all.resolve({ scope: 'all', rounds: 90 });
  uncategorized.resolve({ scope: 'uncategorized', rounds: 9 });
  await Promise.all([a, u]);
  assert.deepEqual(f.values.detailed, { scope: 'category:7', rounds: 2 });
});

test('replacing a scope invalidates both endpoint results and old loading completions', async () => {
  const f = fixture();
  const oldHeat = Promise.withResolvers(),
    nextHeat = Promise.withResolvers();
  const old = f.query('heatmap', oldHeat.promise);
  f.requests.invalidate();
  const next = f.query('heatmap', nextHeat.promise);
  const detail = f.query('detailed', Promise.resolve({ scope: 'category:8' }));
  oldHeat.resolve({ scope: 'all' });
  await old;
  assert.equal(f.loading.heatmap, true);
  assert.equal(f.values.heatmap, null);
  nextHeat.resolve({ scope: 'category:8' });
  await Promise.all([next, detail]);
  assert.equal(f.values.detailed.scope, f.values.heatmap.scope);
});

test('a stale error is ignored while a current query error is retained as an error', async () => {
  const f = fixture();
  const old = Promise.withResolvers(),
    current = Promise.withResolvers();
  const a = f.query('detailed', old.promise);
  f.requests.invalidate();
  const b = f.query('detailed', current.promise);
  old.reject('old scope failure');
  await a;
  assert.equal(f.errors.detailed, null);
  assert.equal(f.loading.detailed, true);
  current.reject('database unavailable');
  await b;
  assert.equal(f.errors.detailed, 'database unavailable');
  assert.equal(f.values.detailed, null);
  assert.equal(f.loading.detailed, false);
});

test('returning to the original scope still rejects its earlier request', async () => {
  const f = fixture();
  const old = Promise.withResolvers();
  const a = f.query('heatmap', old.promise);
  f.requests.invalidate();
  f.requests.invalidate();
  await f.query('heatmap', Promise.resolve({ scope: 'all', seconds: 1234 }));
  old.resolve({ scope: 'all', seconds: 1 });
  await a;
  assert.equal(f.values.heatmap.seconds, 1234);
});
