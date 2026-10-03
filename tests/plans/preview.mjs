// Browser-only UI verification. No Rust process, real preferences or session database.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { mount } from 'svelte';
import { get } from 'svelte/store';
import { settings } from '../../src/lib/stores/settings';

const params = new URLSearchParams(location.search);
const kind = params.get('window') ?? 'main';
if (params.has('host')) {
  const iframe = document.createElement('iframe');
  const child = new URL(location.href);
  child.searchParams.delete('host');
  iframe.src = child.href;
  iframe.width = params.get('width') ?? (kind === 'settings' ? '660' : '360');
  iframe.height = params.get('height') ?? (kind === 'settings' ? '600' : '478');
  iframe.style.border = '1px solid gray';
  document.getElementById('app').append(iframe);
} else {
  mockWindows(kind);
  const theme = await (
    await fetch(`/themes/${params.get('theme') === 'light' ? 'pomotroid-light' : 'pomotroid'}.json`)
  ).json();
  let saved = {
    ...get(settings),
    language: params.get('lang') ?? 'zh',
    theme_mode: 'light',
    theme_light: theme.name,
  };
  const working = {
    time_work_secs: 5400,
    time_short_break_secs: 339,
    time_long_break_secs: 1800,
    long_break_interval: 4,
    short_breaks_enabled: false,
    long_breaks_enabled: true,
    auto_start_work: true,
    auto_start_break: true,
  };
  const standard = {
    ...working,
    time_work_secs: 1500,
    time_short_break_secs: 300,
    time_long_break_secs: 900,
    short_breaks_enabled: true,
    auto_start_work: false,
    auto_start_break: false,
  };
  let timer = {
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
    captured_at_ms: new Date(2026, 9, 3, 23, 0).getTime(),
    stop_after_round: false,
    stopped_after_round: false,
  };
  let data = {
    book: {
      plans: [
        { id: 'plan-1', name: 'Current plan', initial_name: true, config: working },
        { id: 'plan-2', name: '标准番茄', initial_name: false, config: standard },
      ],
      selected_id: 'plan-1',
      working,
    },
    active_id: 'plan-1',
    pending_id: null,
    pending_revision: 1,
    modified: false,
    timer,
  };
  if (params.get('scenario') === 'arranged') {
    data.pending_id = 'plan-2';
    data.book.selected_id = 'plan-2';
    data.book.working = standard;
    timer.stop_after_round = true;
  }
  if (params.get('scenario') === 'paused') {
    timer.is_running = false;
    timer.is_paused = true;
  }
  if (params.get('scenario') === 'idle') {
    timer.is_running = false;
    timer.has_started = false;
    timer.elapsed_secs = 0;
  }
  saved = { ...saved, ...data.book.working };
  const calls = [];
  mockIPC(
    async (command, args) => {
      calls.push({ command, args });
      if (command === 'settings_get') return saved;
      if (command === 'themes_list') return [{ ...theme, is_custom: false }];
      if (command === 'timer_plans_get') return structuredClone(data);
      if (command === 'timer_get_state') return structuredClone(timer);
      if (command === 'timer_plans_take_focus') return false;
      if (command === 'timer_plans_action') {
        const a = args.action;
        if (['save_as', 'rename', 'template'].includes(a.kind)) {
          a.name = a.name.trim();
          if (!a.name || [...a.name].length > 24) throw 'plan_invalid_name';
          if (
            data.book.plans.some(
              (p) =>
                p.name.toLowerCase() === a.name.toLowerCase() &&
                p.id !== (a.kind === 'rename' ? a.id : '')
            )
          )
            throw 'plan_duplicate_name';
        }
        if (a.kind === 'edit')
          data.book.working = {
            ...data.book.working,
            [a.key === 'work_rounds' ? 'long_break_interval' : a.key]: /^(true|false)$/.test(
              a.value
            )
              ? a.value === 'true'
              : Number(a.value),
          };
        if (a.kind === 'save')
          data.book.plans.find((p) => p.id === data.book.selected_id).config = {
            ...data.book.working,
          };
        if (a.kind === 'save_as' || a.kind === 'template') {
          const id = `plan-${data.book.plans.length + 1}`;
          data.book.plans.push({
            id,
            name: a.name,
            initial_name: false,
            config: a.kind === 'save_as' ? { ...data.book.working } : standard,
          });
          if (a.kind === 'save_as') data.book.selected_id = id;
        }
        if (a.kind === 'rename')
          Object.assign(
            data.book.plans.find((p) => p.id === a.id),
            { name: a.name, initial_name: false }
          );
        if (a.kind === 'delete') data.book.plans = data.book.plans.filter((p) => p.id !== a.id);
        if (a.kind === 'select') {
          data.book.selected_id = a.id;
          data.book.working = { ...data.book.plans.find((p) => p.id === a.id).config };
        }
        if (['select', 'edit', 'save_as'].includes(a.kind)) {
          if (timer.has_started) data.pending_id = data.book.selected_id;
          else {
            data.active_id = data.book.selected_id;
            timer.total_secs = data.book.working.time_work_secs;
          }
        }
        if (a.kind === 'cancel_pending') {
          data.pending_id = null;
          data.book.selected_id = data.active_id;
          data.book.working = { ...data.book.plans.find((p) => p.id === data.active_id).config };
        }
        if (a.kind === 'stop_after') timer.stop_after_round = a.enabled;
        if (a.kind === 'apply_now') {
          data.active_id = data.pending_id;
          data.pending_id = null;
          timer = {
            ...timer,
            total_secs: data.book.working.time_work_secs,
            elapsed_secs: 0,
            round_id: timer.round_id + 1,
            stop_after_round: false,
          };
        }
        data.modified =
          JSON.stringify(data.book.working) !==
          JSON.stringify(data.book.plans.find((p) => p.id === data.book.selected_id).config);
        timer.revision++;
        data.pending_revision++;
        data.timer = timer;
        saved = { ...saved, ...data.book.working };
        await emit('timer:state', structuredClone(timer));
        await emit('plans:changed', structuredClone(data));
        await emit('settings:changed', saved);
        return structuredClone(data);
      }
      if (command === 'timer_toggle') {
        timer.has_started = true;
        timer.is_running = !timer.is_running;
        timer.is_paused = !timer.is_running;
        timer.revision++;
        await emit('timer:state', structuredClone(timer));
        return;
      }
      if (
        command.startsWith('plugin:log|') ||
        command === 'aux_window_ready' ||
        command === 'plugin:window|show' ||
        command === 'notification_show' ||
        command === 'timer_plans_manage'
      )
        return;
      if (command === 'plugin:window|is_maximized') return false;
      throw new Error(`Unexpected fixture IPC: ${command}`);
    },
    { shouldMockEvents: true }
  );
  const Page =
    kind === 'settings'
      ? (await import('../../src/routes/settings/+page.svelte')).default
      : (await import('../../src/routes/+page.svelte')).default;
  mount(Page, { target: document.getElementById('app') });
  window.plansFixture = { calls, snapshot: () => structuredClone(data) };
}
