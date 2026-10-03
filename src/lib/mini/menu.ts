import { Menu, MenuItem, CheckMenuItem, PredefinedMenuItem } from '@tauri-apps/api/menu';
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
    restore: () => void;
    hide: () => void;
    exit: () => void;
  }
) {
  const items: (MenuItem | CheckMenuItem | PredefinedMenuItem)[] = [];
  let menu: Menu | undefined;
  async function item(text: string, enabled: boolean, action: () => void) {
    items.push(await MenuItem.new({ text, enabled, action }));
  }
  try {
    await item(
      state?.is_running ? m.mini_pause() : state?.is_paused ? m.mini_resume() : m.mini_start(),
      !!state,
      actions.toggle
    );
    await item(m.mini_restart(), !!state, actions.restart);
    await item(m.mini_skip(), !!state, actions.skip);
    items.push(await PredefinedMenuItem.new({ item: 'Separator' }));
    items.push(
      await CheckMenuItem.new({
        text: m.mini_top(),
        checked: info.always_on_top,
        action: actions.top,
      })
    );
    await item(m.mini_restore(), true, actions.restore);
    await item(m.mini_hide(), info.tray_available, actions.hide);
    items.push(await PredefinedMenuItem.new({ item: 'Separator' }));
    await item(m.mini_exit(), true, actions.exit);
    menu = await Menu.new({ items });
    // On Windows this resolves when the native popup closes (including Escape).
    await menu.popup();
  } finally {
    await menu?.close().catch(() => {});
    await Promise.all(items.map((entry) => entry.close().catch(() => {})));
  }
}
