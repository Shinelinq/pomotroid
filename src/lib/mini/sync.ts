import type { TimerState } from '$lib/types';

type Unlisten = () => void;
type Listener<T> = (callback: (value: T) => void) => Promise<Unlisten>;
export interface MiniTimerApi {
  getTimerState(): Promise<TimerState>;
  onTimerState?: Listener<TimerState>;
  onTimerTick: Listener<{ elapsed_secs: number; total_secs: number }>;
  onTimerPaused: Listener<{ elapsed_secs: number }>;
  onTimerResumed: Listener<{ elapsed_secs: number }>;
  onRoundChange: Listener<TimerState>;
  onTimerReset: Listener<TimerState>;
}

/** Event subscriptions are established before reading a snapshot. No local clock runs here. */
export function syncMiniTimer(
  api: MiniTimerApi,
  update: (state: TimerState, smooth: boolean) => void
) {
  const cleanups: Unlisten[] = [];
  let disposed = false;
  let revision = 0;
  let state: TimerState | null = null;
  function receive(patch: Partial<TimerState>, full = false, tick = false) {
    ++revision;
    if (disposed) return;
    if (full && state && patch.revision !== undefined && patch.revision < state.revision) return;
    if (!state && !full) return;
    const previous = state;
    state = full ? (patch as TimerState) : { ...state!, ...patch };
    const smooth = !!(
      tick &&
      previous?.is_running &&
      state.total_secs === previous.total_secs &&
      state.round_type === previous.round_type &&
      state.elapsed_secs === previous.elapsed_secs + 1
    );
    update(state, smooth);
  }
  async function keep(promise: Promise<Unlisten>) {
    const unlisten = await promise;
    if (disposed) unlisten();
    else cleanups.push(unlisten);
  }
  function dispose() {
    disposed = true;
    for (const unlisten of cleanups.splice(0)) unlisten();
  }
  const ready = (async () => {
    try {
      await Promise.all(
        api.onTimerState
          ? [keep(api.onTimerState((p) => receive(p, true, true)))]
          : [
              keep(
                api.onTimerTick((p) =>
                  receive({ ...p, is_running: true, is_paused: false }, false, true)
                )
              ),
              keep(api.onTimerPaused((p) => receive({ ...p, is_running: false, is_paused: true }))),
              keep(
                api.onTimerResumed((p) => receive({ ...p, is_running: true, is_paused: false }))
              ),
              keep(api.onRoundChange((p) => receive(p, true))),
              keep(api.onTimerReset((p) => receive(p, true))),
            ]
      );
      while (!disposed) {
        const before = revision;
        const snapshot = await api.getTimerState();
        if (disposed) return;
        // An event raced the IPC read: do not overwrite it with an older snapshot.
        if (revision !== before) continue;
        state = snapshot;
        update(snapshot, false);
        return;
      }
    } catch (error) {
      dispose();
      throw error;
    }
  })();
  return { ready, dispose };
}

export function miniDisplay(state: TimerState) {
  const remaining = Math.max(state.total_secs - state.elapsed_secs, 0);
  const ratio = state.total_secs > 0 ? Math.min(1, Math.max(0, remaining / state.total_secs)) : 0;
  const text = `${String(Math.floor(remaining / 60)).padStart(2, '0')}:${String(remaining % 60).padStart(2, '0')}`;
  return { remaining, ratio, text, fontSize: Math.min(24, 132 / text.length) };
}
