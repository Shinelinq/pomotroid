import type { TimerState } from '../types';

/** Derived only from a backend snapshot, with no frontend clock or countdown. */
export function eta(state: TimerState, locale: string) {
  const remaining = Math.max(0, state.total_secs - state.elapsed_secs);
  if (!state.has_started) return { kind: 'waiting' as const };
  if (!state.is_running)
    return {
      kind: 'paused' as const,
      minutes: Math.floor(remaining / 60),
      seconds: remaining % 60,
    };
  const captured = new Date(state.captured_at_ms);
  const finish = new Date(state.captured_at_ms + remaining * 1000);
  const day = (date: Date) => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate());
  const days = Math.round((day(finish) - day(captured)) / 86400000);
  const time = new Intl.DateTimeFormat(locale, {
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).format(finish);
  const date = new Intl.DateTimeFormat(locale, { month: 'numeric', day: 'numeric' }).format(finish);
  return { kind: 'running' as const, days, time, date };
}
