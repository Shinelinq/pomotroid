import { Menu, MenuItem, CheckMenuItem, PredefinedMenuItem, Submenu } from '@tauri-apps/api/menu';
import { nativePlanItems } from '$lib/plans/native';
import { onMiniStatus, onTimerState } from '$lib/ipc';
import * as m from '$paraglide/messages.js';
import type { MiniInfo, TimerState } from '$lib/types';

export async function popupMiniMenu(
  state: TimerState | null,
  info: MiniInfo,
  actions: {
    toggle: () => void;
    restart: () => void;
    skip: () => void;
    top: () => void;
    snap: () => void;
    lock: () => void;
    restore: () => void;
    hide: () => void;
    exit: () => void;
  }
) {
  const items: (MenuItem | CheckMenuItem | PredefinedMenuItem | Submenu)[] = [];
  const cleanups: (() => void)[] = [];
  let menu: Menu | undefined;
  let plans: Awaited<ReturnType<typeof nativePlanItems>> | undefined;
  let lines: MenuItem[] = [];
  let disposed = false;
  let updates = Promise.resolve();
  async function item(text: string, enabled: boolean, action?: () => void) {
    const entry = await MenuItem.new({ text, enabled, action });
    items.push(entry);
    return entry;
  }
  const separator = async () => items.push(await PredefinedMenuItem.new({ item: 'Separator' }));
  try {
    for (const line of info.menu_lines) lines.push(await item(line, false));
    const toggle = await item(
      state?.is_running ? m.mini_pause() : state?.is_paused ? m.mini_resume() : m.mini_start(),
      !!state,
      actions.toggle
    );
    await item(m.mini_restart(), !!state, actions.restart);
    await item(m.mini_skip(), !!state, actions.skip);
    plans = await nativePlanItems();
    items.push(plans.stopItem);
    await separator();
    items.push(plans.planMenu, plans.categoryMenu);
    await separator();
    for (const [text, checked, action] of [
      [m.mini_top(), info.always_on_top, actions.top],
      [m.mini_snap(), info.snap_enabled, actions.snap],
      [m.mini_lock(), info.position_locked, actions.lock],
    ] as const)
      items.push(await CheckMenuItem.new({ text, checked, action }));
    await separator();
    await item(m.mini_restore(), true, actions.restore);
    await item(m.mini_hide(), info.tray_available, actions.hide);
    await separator();
    await item(m.mini_exit(), true, actions.exit);
    menu = await Menu.new({ items });
    cleanups.push(
      await onTimerState((value) => {
        updates = updates
          .then(async () => {
            if (disposed) return;
            await toggle.setText(
              value.is_running ? m.mini_pause() : value.is_paused ? m.mini_resume() : m.mini_start()
            );
          })
          .catch(() => {});
      })
    );
    cleanups.push(
      await onMiniStatus((value) => {
        updates = updates
          .then(async () => {
            if (disposed || !menu) return;
            // Only changed status/ETA text is emitted by the backend, never a per-second rebuild.
            while (lines.length > value.length) {
              const line = lines.pop()!;
              await menu.remove(line);
              await line.close();
            }
            for (const [index, text] of value.entries()) {
              if (lines[index]) await lines[index].setText(text);
              else {
                const line = await MenuItem.new({ text, enabled: false });
                lines.push(line);
                await menu.insert(line, index);
              }
            }
          })
          .catch(() => {});
      })
    );
    await menu.popup();
  } finally {
    disposed = true;
    cleanups.forEach((stop) => stop());
    await updates;
    await menu?.close().catch(() => {});
    await Promise.all(
      [...new Set([...items, ...lines])]
        .filter((entry) => !plans?.items.includes(entry as Submenu | CheckMenuItem))
        .map((entry) => entry.close().catch(() => {}))
    );
    await plans?.dispose();
  }
}
