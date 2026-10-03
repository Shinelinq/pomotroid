import { get, writable } from 'svelte/store';
import { getTimerPlans, onTimerPlansChanged, onTimerState, timerPlanAction } from '$lib/ipc';
import { timerState } from '$lib/stores/timer';
import { settings } from '$lib/stores/settings';
import type { PlanAction, PlanState, TimerState } from '$lib/types';

export const plans = writable<PlanState | null>(null);
export const planLoadError = writable(false);
let planRevision = -1;

function acceptTimer(value: TimerState) {
  timerState.update((old) => (value.revision >= old.revision ? value : old));
}
function accept(value: PlanState) {
  if (value.timer.revision < planRevision) return;
  planRevision = value.timer.revision;
  acceptTimer(value.timer);
  plans.set({ ...value, timer: get(timerState) });
  settings.update((old) => ({ ...old, ...value.book.working }));
}
export async function actOnPlan(action: PlanAction) {
  const result = await timerPlanAction(action);
  accept(result);
  return result;
}

/** Subscribe before hydration; delayed IPC responses never roll state backwards. */
export function connectPlans() {
  let disposed = false;
  const cleanups: (() => void)[] = [];
  async function keep(promise: Promise<() => void>) {
    const stop = await promise;
    if (disposed) stop();
    else cleanups.push(stop);
  }
  const ready = (async () => {
    try {
      await Promise.all([
        keep(
          onTimerPlansChanged((value) => {
            if (!disposed) accept(value);
          })
        ),
        keep(
          onTimerState((value) => {
            if (!disposed) acceptTimer(value);
          })
        ),
      ]);
      const result = await getTimerPlans();
      if (!disposed) {
        accept(result);
        planLoadError.set(false);
      }
    } catch {
      if (!disposed) planLoadError.set(true);
    }
  })();
  return {
    ready,
    dispose() {
      disposed = true;
      cleanups.splice(0).forEach((stop) => stop());
    },
  };
}
