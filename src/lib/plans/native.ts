import { Menu, MenuItem, CheckMenuItem, Submenu } from '@tauri-apps/api/menu';
import { error as logError } from '@tauri-apps/plugin-log';
import {
  getTimerPlans,
  timerPlanAction,
  manageTimerPlans,
  onTimerState,
  onTimerPlansChanged,
} from '$lib/ipc';
import { planName } from './format';
import { nativeCategoryMenu } from '$lib/categories/native';
import * as m from '$paraglide/messages.js';

type Entry = MenuItem | CheckMenuItem | Submenu;

/** Reused by mini and the main window's compact-mode native context menu. */
export async function nativePlanItems() {
  const resources: Entry[] = [];
  const cleanups: (() => void)[] = [];
  let disposed = false;
  let categoryMenu: Awaited<ReturnType<typeof nativeCategoryMenu>> | undefined;
  const state = await getTimerPlans();
  let snapshot = state.timer;
  const roundId = snapshot.round_id;
  const planItems = new Map<string, MenuItem>();
  const run = (action: () => Promise<unknown>) => {
    void action().catch((error) => logError(`[plans] ${error}`).catch(() => {}));
  };
  try {
    const children: Entry[] = [];
    for (const plan of state.book.plans) {
      const suffix = [
        plan.id === state.active_id ? m.plan_active() : '',
        plan.id === state.pending_id ? m.plan_next() : '',
      ]
        .filter(Boolean)
        .join(' · ');
      const item = await MenuItem.new({
        text: `${planName(plan)}${suffix ? ` · ${suffix}` : ''}`,
        action: () => run(() => timerPlanAction({ kind: 'select', id: plan.id })),
      });
      children.push(item);
      resources.push(item);
      planItems.set(plan.id, item);
    }
    if (state.pending_id) {
      const cancel = await MenuItem.new({
        text: m.plan_cancel_pending(),
        action: () => run(() => timerPlanAction({ kind: 'cancel_pending' })),
      });
      children.push(cancel);
      resources.push(cancel);
    }
    const manage = await MenuItem.new({
      text: m.plan_manage(),
      action: () => run(manageTimerPlans),
    });
    children.push(manage);
    resources.push(manage);
    const submenu = await Submenu.new({ text: m.plan_title(), items: children });
    resources.push(submenu);
    const stop = await CheckMenuItem.new({
      text: state.timer.has_started
        ? m.round_stop()
        : `${m.round_stop()} · ${m.round_stop_disabled()}`,
      enabled: state.timer.has_started,
      checked: state.timer.stop_after_round,
      action: () =>
        run(() =>
          timerPlanAction({
            kind: 'stop_after',
            round_id: roundId,
            enabled: !snapshot.stop_after_round,
          })
        ),
    });
    resources.push(stop);
    async function updateTimer(value: typeof snapshot) {
      if (disposed || value.revision < snapshot.revision) return;
      snapshot = value;
      const sameRound = value.round_id === roundId;
      await Promise.all([
        stop.setChecked(sameRound && value.stop_after_round),
        stop.setEnabled(sameRound && value.has_started),
        stop.setText(
          sameRound && value.has_started
            ? m.round_stop()
            : `${m.round_stop()} · ${sameRound ? m.round_stop_disabled() : m.round_changed()}`
        ),
      ]);
    }
    cleanups.push(await onTimerState((value) => run(() => updateTimer(value))));
    cleanups.push(
      await onTimerPlansChanged((value) =>
        run(async () => {
          await updateTimer(value.timer);
          for (const [id, item] of planItems) {
            const plan = value.book.plans.find((p) => p.id === id);
            if (!plan) {
              await item.setEnabled(false);
              continue;
            }
            const suffix = [
              id === value.active_id ? m.plan_active() : '',
              id === value.pending_id ? m.plan_next() : '',
            ]
              .filter(Boolean)
              .join(' · ');
            await item.setText(`${planName(plan)}${suffix ? ` · ${suffix}` : ''}`);
          }
        })
      )
    );
    await updateTimer((await getTimerPlans()).timer);
    categoryMenu = await nativeCategoryMenu();
    return {
      planMenu: submenu,
      stopItem: stop,
      categoryMenu: categoryMenu.submenu,
      items: [submenu, stop, categoryMenu.submenu],
      dispose: async () => {
        disposed = true;
        cleanups.splice(0).forEach((stop) => stop());
        await categoryMenu?.dispose();
        return Promise.all(resources.map((item) => item.close().catch(() => {})));
      },
    };
  } catch (error) {
    disposed = true;
    cleanups.splice(0).forEach((stop) => stop());
    await categoryMenu?.dispose();
    await Promise.all(resources.map((item) => item.close().catch(() => {})));
    throw error;
  }
}

let open = false;
export async function popupPlanControls() {
  if (open) return;
  open = true;
  let menu: Menu | undefined;
  let items: Awaited<ReturnType<typeof nativePlanItems>> | undefined;
  try {
    items = await nativePlanItems();
    menu = await Menu.new({ items: items.items });
    await menu.popup();
  } catch (error) {
    void logError(`[plans] ${error}`).catch(() => {});
  } finally {
    await menu?.close().catch(() => {});
    await items?.dispose();
    open = false;
  }
}
