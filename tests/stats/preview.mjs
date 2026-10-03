// Development-only browser fixture. All IPC is mocked; no Rust process or database is used.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { mount } from 'svelte';
import { get } from 'svelte/store';
import { settings } from '../../src/lib/stores/settings';
import { dateKey, shiftDate } from '../../src/lib/components/stats/stats';

const params = new URLSearchParams(location.search);
const themes = await Promise.all(
  ['pomotroid', 'pomotroid-light'].map(async (name) => ({
    ...(await (await fetch(`/themes/${name}.json`)).json()),
    is_custom: false,
  }))
);
let saved = {
  ...get(settings),
  language: params.get('lang') ?? 'zh',
  theme_mode: 'light',
  theme_light: themes[params.get('theme') === 'light' ? 1 : 0].name,
};
const today = dateKey(new Date());
const windowKind = params.get('window') === 'settings' ? 'settings' : 'stats';
let maximized = false;
let scenario = params.get('scenario') ?? 'mixed';
let failing = scenario === 'error';
let calls = { detailed: 0, heatmap: 0 };
const queuedDetailed = [];
const week = [
  { date: shiftDate(today, -6), rounds: 2, started_rounds: 3, focus_secs: 4200 },
  { date: shiftDate(today, -4), rounds: 0, started_rounds: 2, focus_secs: 0 },
  { date: shiftDate(today, -3), rounds: 4, started_rounds: 4, focus_secs: 7200 },
  { date: shiftDate(today, -1), rounds: 1, started_rounds: 1, focus_secs: 2700 },
  { date: today, rounds: 3, started_rounds: 4, focus_secs: 6300 },
];
const entries = [
  { date: '2000-02-29', count: 1, focus_secs: 3600 },
  { date: `${new Date().getFullYear() - 1}-01-01`, count: 2, focus_secs: 4200 },
  ...week
    .filter((day) => day.rounds > 0)
    .map((day) => ({ date: day.date, count: day.rounds, focus_secs: day.focus_secs })),
];
function detailedData() {
  const active =
    scenario === 'empty' || scenario === 'error'
      ? []
      : scenario === 'incomplete'
        ? [{ date: today, rounds: 0, started_rounds: 3, focus_secs: 0 }]
        : scenario === 'single'
          ? [week.at(-1)]
          : week;
  const rounds = active.at(-1)?.rounds ?? 0;
  const seconds = active.at(-1)?.focus_secs ?? 0;
  return {
    today: {
      rounds: active.at(-1)?.rounds ?? 0,
      focus_mins: (active.at(-1)?.focus_secs ?? 0) / 60,
      completion_rate: active.length ? 0.75 : null,
      by_hour: Array.from({ length: 24 }, (_, i) =>
        i === 9 ? Math.max(0, rounds - 1) : i === 14 ? Math.min(1, rounds) : 0
      ),
      by_hour_focus_secs: Array.from({ length: 24 }, (_, i) =>
        i === 9 && rounds > 1
          ? Math.max(0, seconds - 900)
          : i === 14 && rounds > 0
            ? rounds > 1
              ? Math.min(900, seconds)
              : seconds
            : 0
      ),
    },
    week: active,
    streak: { current: active.some((d) => d.rounds) ? 2 : 0, longest: 5 },
  };
}
function heatmapData() {
  const active = (
    ['empty', 'incomplete', 'error'].includes(scenario)
      ? []
      : scenario === 'single'
        ? entries.slice(-1)
        : entries
  ).map((entry) => ({ ...entry }));
  if (scenario === 'long')
    active[0] = {
      ...active[0],
      focus_secs: 1123500 - active.slice(1).reduce((sum, day) => sum + day.focus_secs, 0),
    };
  const seconds = active.reduce((sum, day) => sum + day.focus_secs, 0);
  return {
    entries: active,
    total_rounds: active.reduce((sum, day) => sum + day.count, 0),
    total_hours: Math.floor(seconds / 3600),
    total_focus_secs: seconds,
    longest_streak: active.length ? 5 : 0,
  };
}
mockWindows(windowKind);
mockIPC(
  async (command, args) => {
    if (command === 'aux_window_ready') return;
    if (command === 'plugin:window|is_maximized') return maximized;
    if (command === 'plugin:window|toggle_maximize') {
      maximized = !maximized;
      await emit('tauri://resize', { width: innerWidth, height: innerHeight });
      return;
    }
    if (command === 'tray_supported' || command === 'accessibility_trusted') return true;
    if (command === 'audio_get_custom_info')
      return { work_alert: null, short_break_alert: null, long_break_alert: null };
    if (command === 'app_version') return 'test-fixture';
    if (command === 'check_update') return null;
    if (command === 'get_log_dir') return 'test-only';
    if (command === 'settings_set') {
      const { key, value } = args;
      const parsed =
        typeof saved[key] === 'boolean'
          ? value === 'true'
          : typeof saved[key] === 'number'
            ? Number(value)
            : value;
      saved = { ...saved, [key]: key === 'volume' ? Number(value) / 100 : parsed };
      await emit('settings:changed', saved);
      return saved;
    }
    if (command === 'settings_get') return saved;
    if (command === 'themes_list') return themes;
    if (command === 'stats_get_detailed' || command === 'stats_get_heatmap') {
      calls[command === 'stats_get_detailed' ? 'detailed' : 'heatmap']++;
      document.body.dataset.detailedRequests = String(calls.detailed);
      document.body.dataset.heatmapRequests = String(calls.heatmap);
      if (command === 'stats_get_detailed' && queuedDetailed.length) {
        const { data, delay } = queuedDetailed.shift();
        await new Promise((resolve) => setTimeout(resolve, delay));
        return data;
      }
      const result = command === 'stats_get_detailed' ? detailedData() : heatmapData();
      await new Promise((resolve) => setTimeout(resolve, 80));
      if (failing) throw new Error('Fixture query failure');
      return result;
    }
    if (
      command.startsWith('plugin:log|') ||
      command === 'plugin:window|show' ||
      command === 'plugin:window|close'
    )
      return;
    throw new Error(`Unexpected fixture IPC: ${command}`);
  },
  { shouldMockEvents: true }
);

const fixture = {
  calls,
  async scenario(value) {
    scenario = value;
    failing = value === 'error';
    await emit('sessions:cleared');
  },
  async theme(light) {
    saved = { ...saved, theme_light: themes[light ? 1 : 0].name };
    await emit('settings:changed', saved);
  },
  async language(language) {
    saved = { ...saved, language };
    await emit('settings:changed', saved);
  },
};
const { default: StatsPage } =
  windowKind === 'settings'
    ? await import('../../src/routes/settings/+page.svelte')
    : await import('../../src/routes/stats/+page.svelte');
mount(StatsPage, { target: document.getElementById('app') });

// Optional fixture controls are outside the product component and absent from normal screenshots.
if (params.has('controls')) {
  const controls = document.createElement('details');
  controls.id = 'fixture-controls';
  controls.style.cssText =
    'position:fixed;z-index:20000;right:20px;bottom:2px;font:12px sans-serif;background:var(--color-background-light);color:var(--color-foreground);padding:4px';
  controls.innerHTML = '<summary>Test controls</summary>';
  const actions = {
    Empty: () => fixture.scenario('empty'),
    Incomplete: () => fixture.scenario('incomplete'),
    Single: () => fixture.scenario('single'),
    Mixed: () => fixture.scenario('mixed'),
    Failure: () => fixture.scenario('error'),
    'Allow retry': () => {
      failing = false;
      scenario = 'mixed';
    },
    Light: () => fixture.theme(true),
    Dark: () => fixture.theme(false),
    German: () => fixture.language('de'),
    Chinese: () => fixture.language('zh'),
    'Request race': async () => {
      scenario = 'mixed';
      const older = detailedData();
      const newer = {
        ...older,
        week: [{ date: today, rounds: 7, started_rounds: 7, focus_secs: 6300 }],
      };
      queuedDetailed.push({ data: older, delay: 700 }, { data: newer, delay: 30 });
      await emit('timer:round-change');
      await emit('timer:round-change');
    },
  };
  for (const [name, action] of Object.entries(actions)) {
    const button = document.createElement('button');
    button.textContent = name;
    button.onclick = action;
    button.style.cssText = 'padding:3px;margin:2px';
    controls.append(button);
  }
  document.body.append(controls);
}
