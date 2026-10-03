import { MenuItem, CheckMenuItem, Submenu } from '@tauri-apps/api/menu';
import { error as logError } from '@tauri-apps/plugin-log';
import {
  getCategories,
  categoryAction,
  onCategoriesChanged,
  onTimerState,
  manageCategories,
} from '$lib/ipc';
import { categoryName } from './format';
import type { CategoryState, TimerState } from '$lib/types';
import * as m from '$paraglide/messages.js';

/** Only a native submenu is added to mini; its geometry and drag handling are untouched. */
export async function nativeCategoryMenu() {
  let data = await getCategories();
  let timer = data.timer;
  let disposed = false;
  const cleanups: (() => void)[] = [];
  let children: (MenuItem | CheckMenuItem)[] = [];
  const submenu = await Submenu.new({ text: m.category_title(), items: [] });
  const report = (error: unknown) => logError(`[categories] ${error}`).catch(() => {});
  const run = (action: () => Promise<unknown>) => {
    void action().catch(report);
  };
  const timerKey = (state: TimerState) =>
    JSON.stringify([
      state.round_id,
      state.round_type,
      state.has_started,
      state.category_id,
      state.next_category_id,
      state.category_pending,
    ]);
  let lastTimerKey = '';
  let queue = Promise.resolve();
  async function redraw() {
    if (disposed) return;
    await submenu.setText(
      timer.round_type === 'work' ? m.category_focus_menu() : m.category_next_menu()
    );
    for (const child of children.splice(0)) {
      await submenu.remove(child);
      await child.close();
    }
    const append = async (item: MenuItem | CheckMenuItem) => {
      children.push(item);
      await submenu.append(item);
    };
    await append(
      await MenuItem.new({
        text:
          timer.round_type === 'work' && timer.has_started
            ? m.category_current({ name: categoryName(data.data.items, timer.category_id) })
            : m.category_next_focus({
                name: categoryName(data.data.items, timer.next_category_id),
              }),
        enabled: false,
      })
    );
    if (timer.round_type === 'work' && timer.has_started) {
      await append(await MenuItem.new({ text: m.category_next_hint(), enabled: false }));
    }
    for (const item of [null, ...data.data.items.filter((c) => !c.archived)]) {
      const id = item?.id ?? null;
      await append(
        await CheckMenuItem.new({
          text: item?.name ?? m.category_uncategorized(),
          checked: timer.next_category_id === id,
          action: () => run(() => categoryAction({ kind: 'select', id })),
        })
      );
    }
    if (timer.category_pending) {
      await append(
        await MenuItem.new({
          text: m.category_pending({ name: categoryName(data.data.items, timer.next_category_id) }),
          enabled: false,
        })
      );
    }
    await append(
      await MenuItem.new({ text: m.category_manage(), action: () => run(manageCategories) })
    );
    lastTimerKey = timerKey(timer);
  }
  function schedule() {
    queue = queue.then(redraw).catch(report);
  }
  function accept(value: CategoryState) {
    if (value.revision < data.revision) return;
    data = value;
    if (value.timer.revision >= timer.revision) timer = value.timer;
    schedule();
  }
  try {
    cleanups.push(await onCategoriesChanged(accept));
    cleanups.push(
      await onTimerState((value) => {
        if (value.revision < timer.revision) return;
        timer = value;
        if (timerKey(value) !== lastTimerKey) schedule();
      })
    );
    accept(await getCategories());
    await queue;
    return {
      submenu,
      async dispose() {
        disposed = true;
        cleanups.splice(0).forEach((stop) => stop());
        await queue;
        await submenu.close().catch(() => {});
        await Promise.all(children.splice(0).map((child) => child.close().catch(() => {})));
      },
    };
  } catch (error) {
    disposed = true;
    cleanups.splice(0).forEach((stop) => stop());
    await queue;
    await submenu.close().catch(() => {});
    await Promise.all(children.map((child) => child.close().catch(() => {})));
    throw error;
  }
}
