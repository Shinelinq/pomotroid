// Test-only records and IPC. No application process, real database or user settings.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { mount } from 'svelte';
import { get } from 'svelte/store';
import { settings } from '../../src/lib/stores/settings';
import { dateKey, shiftDate } from '../../src/lib/components/stats/stats';

const params = new URLSearchParams(location.search);
const kind = params.get('window') ?? 'stats';
if (params.has('host')) {
  const iframe = document.createElement('iframe');
  const child = new URL(location.href);
  child.searchParams.delete('host');
  iframe.src = child.href;
  iframe.width = params.get('width') ?? (kind === 'main' ? '360' : '800');
  iframe.height = params.get('height') ?? (kind === 'main' ? '478' : '650');
  iframe.style.border = '1px solid gray';
  document.getElementById('app').append(iframe);
} else {
  mockWindows(kind);
  const theme = await (
    await fetch(`/themes/${params.get('theme') === 'light' ? 'pomotroid-light' : 'pomotroid'}.json`)
  ).json();
  const saved = {
    ...get(settings),
    language: params.get('lang') ?? 'zh',
    theme_mode: 'light',
    theme_light: theme.name,
  };
  const config = {
    time_work_secs: 5400,
    time_short_break_secs: 300,
    time_long_break_secs: 900,
    long_break_interval: 4,
    short_breaks_enabled: true,
    long_breaks_enabled: true,
    auto_start_work: true,
    auto_start_break: true,
  };
  Object.assign(saved, config);
  const timer = {
    round_type: 'work',
    previous_round_type: '',
    elapsed_secs: 600,
    total_secs: 5400,
    is_running: true,
    is_paused: false,
    has_started: true,
    work_round_number: 2,
    work_rounds_total: 4,
    session_work_count: 2,
    round_id: 1,
    revision: 1,
    captured_at_ms: Date.now(),
    stop_after_round: false,
    stopped_after_round: false,
    category_id: 1,
    next_category_id: 1,
    category_pending: false,
    category_notice_id: null,
    session_id: null,
  };
  const items = [
    { id: 1, name: '阅读', archived: false },
    { id: 2, name: '编程', archived: false },
    { id: 3, name: '旧分类', archived: true },
    { id: 4, name: '暂无记录', archived: false },
  ];
  let categoryRevision = 1;
  let focusCategories = kind === 'settings';
  const plan = {
    book: {
      plans: [
        { id: 'plan-1', name: 'Current plan', initial_name: true, config },
        {
          id: 'plan-2',
          name: '标准番茄',
          initial_name: false,
          config: { ...config, time_work_secs: 1500 },
        },
      ],
      selected_id: 'plan-1',
      working: config,
    },
    active_id: 'plan-1',
    pending_id: null,
    pending_revision: 1,
    modified: false,
    timer,
  };
  if (params.get('scenario') === 'arranged') {
    plan.pending_id = 'plan-2';
    timer.stop_after_round = true;
    timer.next_category_id = 2;
    timer.category_pending = true;
  }
  if (params.get('scenario') === 'break') {
    timer.round_type = 'short-break';
    timer.total_secs = 900;
    timer.elapsed_secs = 100;
    timer.category_id = null;
  }
  const state = () =>
    structuredClone({
      data: { items, selected_id: timer.next_category_id },
      revision: categoryRevision,
      timer,
    });
  const today = dateKey(new Date());
  const rows = [
    { date: today, category: null, secs: 900, completed: true },
    { date: today, category: 1, secs: 1800, completed: true },
    { date: shiftDate(today, -1), category: 1, secs: 1200, completed: true },
    { date: `${new Date().getFullYear() - 2}-03-02`, category: 1, secs: 400, completed: true },
    { date: today, category: 2, secs: 3600, completed: true },
    { date: today, category: 2, secs: 700, completed: false },
    { date: shiftDate(today, -2), category: 2, secs: 330, completed: true },
    { date: shiftDate(today, -1), category: 3, secs: 5400, completed: true },
  ];
  if (params.has('sessions')) {
    rows.push({ date: today, category: 1, secs: 5400, completed: false, start: '09:00:00' });
    timer.session_id = 9;
    rows.push({ date: today, category: null, secs: 339, completed: false, start: '08:00:00' });
    rows.push({
      date: shiftDate(today, -1),
      category: 3,
      secs: 1801,
      completed: true,
      start: '23:50:00',
      end: `${today}T00:25:00`,
    });
    rows.push({
      date: `${new Date().getFullYear() - 1}-12-31`,
      category: 1,
      secs: 90,
      completed: true,
      start: '23:59:00',
      end: `${new Date().getFullYear()}-01-01T00:04:00`,
    });
    if (params.get('sessions') === 'many')
      for (let i = 0; i < 118; i++)
        rows.push({
          date: today,
          category: 1,
          secs: 60 + i,
          completed: true,
          start: `10:${String(Math.floor(i / 60)).padStart(2, '0')}:${String(i % 60).padStart(2, '0')}`,
        });
  }
  rows.forEach((row, index) => {
    row.id = index + 1;
    row.started_at = new Date(`${row.date}T${row.start ?? '09:15:00'}`).getTime() / 1000;
    row.ended_at = row.end
      ? new Date(row.end).getTime() / 1000
      : row.completed
        ? row.started_at + row.secs
        : null;
  });
  let failSessions = params.get('sessions') === 'error';
  let sessionQueries = 0;
  function sessionPage(q) {
    const max = q.cursor?.max_id ?? Math.max(...rows.map((r) => r.id));
    const range = scoped(q.filter)
      .filter(
        (r) =>
          r.date === q.date &&
          (q.hour == null || new Date(r.started_at * 1000).getHours() === q.hour) &&
          r.id <= max
      )
      .sort((a, b) => a.started_at - b.started_at || a.id - b.id);
    const eligible = range.filter(
      (r) =>
        !q.cursor ||
        r.started_at > q.cursor.started_at ||
        (r.started_at === q.cursor.started_at && r.id > q.cursor.id)
    );
    const selected = eligible.slice(0, q.limit ?? 50);
    const last = selected.at(-1);
    return {
      records: selected.map((r) => ({
        id: r.id,
        started_at: r.started_at,
        ended_at: r.ended_at,
        duration_secs: r.secs,
        completed: r.completed,
        category_id: r.category,
        category_name: items.find((c) => c.id === r.category)?.name ?? null,
        category_archived: items.find((c) => c.id === r.category)?.archived ?? false,
      })),
      summary: {
        recorded: range.length,
        completed: range.filter((r) => r.completed).length,
        focus_secs: range.filter((r) => r.completed).reduce((s, r) => s + r.secs, 0),
      },
      next_cursor:
        eligible.length > selected.length
          ? {
              date: q.date,
              hour: q.hour ?? null,
              filter: q.filter,
              started_at: last.started_at,
              id: last.id,
              max_id: max,
            }
          : null,
    };
  }
  function scoped(filter = { kind: 'all' }) {
    if (filter.kind === 'all') return rows;
    if (filter.kind === 'uncategorized') return rows.filter((r) => r.category === null);
    if (!items.some((c) => c.id === filter.category_id)) throw 'category_missing';
    return rows.filter((r) => r.category === filter.category_id);
  }
  function group(records) {
    const days = new Map();
    for (const row of records) {
      const day = days.get(row.date) ?? {
        date: row.date,
        rounds: 0,
        started_rounds: 0,
        focus_secs: 0,
      };
      day.started_rounds++;
      if (row.completed) {
        day.rounds++;
        day.focus_secs += row.secs;
      }
      days.set(row.date, day);
    }
    return [...days.values()].sort((a, b) => a.date.localeCompare(b.date));
  }
  function streak(records) {
    const days = group(records.filter((r) => r.completed)).map((r) => r.date);
    let longest = 0,
      run = 0,
      previous;
    for (const day of days) {
      run = previous && shiftDate(previous, 1) === day ? run + 1 : 1;
      longest = Math.max(run, longest);
      previous = day;
    }
    return { current: previous === today || previous === shiftDate(today, -1) ? run : 0, longest };
  }
  function detailed(filter) {
    const records = scoped(filter),
      daily = records.filter((r) => r.date === today),
      completed = daily.filter((r) => r.completed);
    const secs = completed.reduce((n, r) => n + r.secs, 0);
    return {
      today: {
        rounds: completed.length,
        focus_mins: Math.floor((secs + 30) / 60),
        completion_rate: daily.length ? completed.length / daily.length : null,
        by_hour: Array.from(
          { length: 24 },
          (_, h) => completed.filter((r) => new Date(r.started_at * 1000).getHours() === h).length
        ),
        by_hour_focus_secs: Array.from({ length: 24 }, (_, h) =>
          completed
            .filter((r) => new Date(r.started_at * 1000).getHours() === h)
            .reduce((n, r) => n + r.secs, 0)
        ),
      },
      week: group(records.filter((r) => r.date >= shiftDate(today, -6) && r.date <= today)),
      streak: streak(records),
    };
  }
  function heatmap(filter) {
    const records = scoped(filter).filter((r) => r.completed);
    const secs = records.reduce((n, r) => n + r.secs, 0);
    return {
      entries: group(records).map((r) => ({
        date: r.date,
        count: r.rounds,
        focus_secs: r.focus_secs,
      })),
      total_rounds: records.length,
      total_hours: Math.floor(secs / 3600),
      total_focus_secs: secs,
      longest_streak: streak(records).longest,
    };
  }
  const calls = [];
  mockIPC(
    async (command, args) => {
      calls.push({ command, args });
      if (command === 'settings_get') return saved;
      if (command === 'themes_list') return [{ ...theme, is_custom: false }];
      if (command === 'timer_get_state') return structuredClone(timer);
      if (command === 'timer_plans_get') return structuredClone(plan);
      if (command === 'categories_get') return state();
      if (command === 'categories_take_focus') {
        const focus = focusCategories;
        focusCategories = false;
        return focus;
      }
      if (command === 'timer_plans_take_focus') return false;
      if (command === 'categories_action') {
        const a = args.action;
        if (a.kind === 'create' || a.kind === 'rename') {
          a.name = a.name.trim();
          if (!a.name || [...a.name].length > 24) throw 'category_invalid_name';
          if (items.some((c) => c.name.toLowerCase() === a.name.toLowerCase() && c.id !== a.id))
            throw 'category_duplicate_name';
          if (a.name === '未分类') throw 'category_reserved_name';
        }
        if (a.kind === 'create')
          items.push({
            id: Math.max(...items.map((c) => c.id)) + 1,
            name: a.name,
            archived: false,
          });
        if (a.kind === 'rename') items.find((c) => c.id === a.id).name = a.name;
        if (a.kind === 'archive' || a.kind === 'restore') {
          items.find((c) => c.id === a.id).archived = a.kind === 'archive';
          if (a.kind === 'restore' && timer.category_notice_id === a.id)
            timer.category_notice_id = null;
          if (a.kind === 'archive' && timer.next_category_id === a.id) {
            timer.next_category_id = null;
            timer.category_notice_id = a.id;
          }
        }
        if (a.kind === 'select' || a.kind === 'cancel_pending') {
          const id = a.kind === 'select' ? a.id : timer.category_id;
          if (id !== null && items.find((c) => c.id === id)?.archived)
            throw 'category_archived_error';
          timer.next_category_id = id;
          timer.category_notice_id = null;
        }
        if (a.kind === 'dismiss_notice') timer.category_notice_id = null;
        timer.category_pending =
          timer.round_type === 'work' &&
          timer.has_started &&
          timer.category_id !== timer.next_category_id;
        timer.revision++;
        categoryRevision++;
        await emit('timer:state', structuredClone(timer));
        await emit('categories:changed', state());
        return state();
      }
      if (command === 'stats_get_sessions') {
        document.body.dataset.sessionQueries = String(++sessionQueries);
        if (failSessions) throw new Error('test database failure');
        const result = sessionPage(args.query);
        await new Promise((resolve) =>
          setTimeout(
            resolve,
            params.has('slow') && args.query.date === shiftDate(today, -1) ? 800 : 30
          )
        );
        return result;
      }
      if (command === 'stats_get_detailed' || command === 'stats_get_heatmap') {
        const result =
          command === 'stats_get_detailed' ? detailed(args.filter) : heatmap(args.filter);
        await new Promise((resolve) =>
          setTimeout(resolve, params.has('slow') && args.filter?.category_id === 1 ? 1200 : 30)
        );
        return result;
      }
      if (command === 'timer_plans_action') {
        if (args.action.kind === 'stop_after') timer.stop_after_round = args.action.enabled;
        if (args.action.kind === 'cancel_pending') plan.pending_id = null;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
        await emit('plans:changed', structuredClone(plan));
        return structuredClone(plan);
      }
      if (command === 'timer_toggle') {
        timer.is_running = !timer.is_running;
        timer.is_paused = !timer.is_running;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
        return;
      }
      if (
        command.startsWith('plugin:log|') ||
        [
          'aux_window_ready',
          'plugin:window|show',
          'notification_show',
          'categories_manage',
          'timer_plans_manage',
        ].includes(command)
      )
        return;
      if (command === 'plugin:window|is_maximized') return false;
      throw new Error(`Unexpected fixture IPC: ${command}`);
    },
    { shouldMockEvents: true }
  );
  const Page =
    kind === 'main'
      ? (await import('../../src/routes/+page.svelte')).default
      : kind === 'settings'
        ? (await import('../../src/routes/settings/+page.svelte')).default
        : (await import('../../src/routes/stats/+page.svelte')).default;
  mount(Page, { target: document.getElementById('app') });
  if (params.has('controls')) {
    const controls = document.createElement('div');
    controls.style.cssText =
      'position:fixed;bottom:2px;left:2px;z-index:20000;display:flex;gap:4px;font-size:11px';
    const actions = {
      'Test pause': async () => {
        timer.is_running = false;
        timer.is_paused = true;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
      },
      'Test tick': async () => {
        timer.elapsed_secs++;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
      },
      'Test complete': async () => {
        const row = rows.find((r) => r.id === timer.session_id);
        if (row) {
          row.completed = true;
          row.ended_at = Date.now() / 1000;
        }
        timer.session_id = null;
        timer.is_running = false;
        timer.is_paused = false;
        timer.has_started = false;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
        await emit('timer:round-change', structuredClone(timer));
      },
      'Test failure': () => {
        failSessions = true;
      },
      'Test recovery': () => {
        failSessions = false;
      },
    };
    for (const [label, action] of Object.entries(actions)) {
      const button = document.createElement('button');
      button.textContent = label;
      button.onclick = action;
      controls.append(button);
    }
    document.body.append(controls);
  }
  window.categoriesFixture = { calls, snapshot: state };
}
