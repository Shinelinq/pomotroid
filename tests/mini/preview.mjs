// Only in-memory IPC fixtures. No native app, timer engine, session or database is started.
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import { emit } from '@tauri-apps/api/event';
import { mount } from 'svelte';
import { get } from 'svelte/store';
import { settings } from '../../src/lib/stores/settings';

const params = new URLSearchParams(location.search);
const phase = params.get('phase') ?? 'running';
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
let snapshot = {
  round_type: 'work',
  previous_round_type: '',
  elapsed_secs: phase === 'idle' ? 0 : 317,
  total_secs: 1500,
  is_running: !['idle', 'paused'].includes(phase),
  is_paused: phase === 'paused',
  work_round_number: 2,
  work_rounds_total: 4,
  session_work_count: 2,
};
if (phase === 'short')
  snapshot = { ...snapshot, round_type: 'short-break', total_secs: 300, elapsed_secs: 40 };
if (phase === 'long')
  snapshot = { ...snapshot, round_type: 'long-break', total_secs: 900, elapsed_secs: 400 };
if (phase === 'over99') snapshot = { ...snapshot, total_secs: 7200, elapsed_secs: 15 };
const calls = {};
const menuItems = new Map();
let resource = 0;
let closePopup;
let top = true;
mockWindows('mini');
mockIPC(
  async (command, args) => {
    calls[command] = (calls[command] ?? 0) + 1;
    document.body.dataset.calls = JSON.stringify(calls);
    if (command === 'settings_get') return saved;
    if (command === 'themes_list') return themes;
    if (command === 'timer_get_state') {
      if (phase === 'error') throw Error('fixture sync failure');
      return { ...snapshot };
    }
    if (command === 'mini_info') return { always_on_top: top, tray_available: params.has('tray') };
    if (command === 'mini_set_top') {
      top = args.value;
      return;
    }
    if (command.startsWith('mini_')) {
      document.body.dataset.lastWindowCommand = command;
      return;
    }
    if (command === 'timer_toggle') {
      await new Promise((resolve) => setTimeout(resolve, 80));
      const running = snapshot.is_running;
      snapshot = { ...snapshot, is_running: !running, is_paused: running };
      await emit(running ? 'timer:paused' : 'timer:resumed', {
        elapsed_secs: snapshot.elapsed_secs,
      });
      return;
    }
    if (command === 'timer_skip') {
      snapshot = { ...snapshot, round_type: 'short-break', elapsed_secs: 0, total_secs: 300 };
      await emit('timer:round-change', snapshot);
      return;
    }
    if (command === 'timer_restart_round') {
      snapshot = { ...snapshot, elapsed_secs: 0 };
      await emit('timer:reset', snapshot);
      return;
    }
    if (command === 'plugin:window|start_dragging') return;
    if (command === 'plugin:menu|new') {
      const id = ++resource;
      menuItems.set(id, { kind: args.kind, ...args.options });
      return [id, `fixture-menu-${id}`];
    }
    if (command === 'plugin:menu|popup') {
      document.body.dataset.menu = JSON.stringify(
        menuItems.get(args.rid).items.map(([id]) => menuItems.get(id))
      );
      return new Promise((resolve) => {
        closePopup = resolve;
      });
    }
    if (command === 'plugin:resources|close') {
      menuItems.delete(args.rid);
      return;
    }
    if (command.startsWith('plugin:log|')) return;
    throw Error(`Unexpected IPC in mini fixture: ${command}`);
  },
  { shouldMockEvents: true }
);
// Model a native popup consuming Escape before the WebView receives it.
document.addEventListener(
  'keydown',
  (event) => {
    if (event.key === 'Escape' && closePopup) {
      event.preventDefault();
      event.stopImmediatePropagation();
      closePopup();
      closePopup = undefined;
    }
  },
  true
);
// Match the existing root layout restriction without removing it for mini.
document.addEventListener('contextmenu', (event) => event.preventDefault(), true);
const { default: MiniPage } = await import('../../src/routes/mini/+page.svelte');
mount(MiniPage, { target: document.getElementById('app') });

if (params.has('controls')) {
  const controls = document.createElement('div');
  controls.style.cssText =
    'position:absolute;left:130px;top:0;display:flex;flex-direction:column;gap:8px';
  const actions = {
    Tick: () => {
      snapshot = { ...snapshot, elapsed_secs: snapshot.elapsed_secs + 1 };
      return emit('timer:tick', snapshot);
    },
    'Long break': () => {
      snapshot = { ...snapshot, round_type: 'long-break', elapsed_secs: 0, total_secs: 900 };
      return emit('timer:round-change', snapshot);
    },
    Theme: () => {
      saved = { ...saved, theme_light: themes[1].name };
      return emit('settings:changed', saved);
    },
    German: () => {
      saved = { ...saved, language: 'de' };
      return emit('settings:changed', saved);
    },
  };
  for (const [name, action] of Object.entries(actions)) {
    const button = document.createElement('button');
    button.textContent = name;
    button.onclick = action;
    controls.append(button);
  }
  document.body.append(controls);
}
