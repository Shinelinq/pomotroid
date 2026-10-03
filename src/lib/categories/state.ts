import { writable } from 'svelte/store';
import { getCategories, categoryAction, onCategoriesChanged, onTimerState } from '$lib/ipc';
import { timerState } from '$lib/stores/timer';
import type { CategoryAction, CategoryState } from '$lib/types';

export const categories = writable<CategoryState | null>(null);
export const categoryLoadError = writable(false);
let revision = -1;

function accept(value: CategoryState, syncTimer = true) {
  if (value.revision >= revision) {
    revision = value.revision;
    categories.set(value);
    categoryLoadError.set(false);
  }
  if (syncTimer)
    timerState.update((old) => (value.timer.revision >= old.revision ? value.timer : old));
}
export async function actOnCategory(action: CategoryAction) {
  const result = await categoryAction(action);
  accept(result);
  return result;
}

/** Per-WebView subscription; catalog events never broadcast or edit Settings. */
export function connectCategories(syncTimer = true) {
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
          onCategoriesChanged((value) => {
            if (!disposed) accept(value, syncTimer);
          })
        ),
        ...(syncTimer
          ? [
              keep(
                onTimerState((value) => {
                  if (!disposed)
                    timerState.update((old) => (value.revision > old.revision ? value : old));
                })
              ),
            ]
          : []),
      ]);
      if (disposed) return;
      const value = await getCategories();
      if (!disposed) accept(value, syncTimer);
    } catch {
      if (!disposed) categoryLoadError.set(true);
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
