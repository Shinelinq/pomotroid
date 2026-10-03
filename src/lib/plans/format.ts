import * as m from '$paraglide/messages.js';
import type { TimerConfig, TimerPlan } from '$lib/types';

export const planName = (plan: TimerPlan) => (plan.initial_name ? m.plan_current() : plan.name);
export const duration = (seconds: number) =>
  `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`;
export function planSummary(c: TimerConfig) {
  return m.plan_summary({
    work: duration(c.time_work_secs),
    short: c.short_breaks_enabled ? duration(c.time_short_break_secs) : m.plan_off(),
    long: c.long_breaks_enabled ? duration(c.time_long_break_secs) : m.plan_off(),
    rounds: c.long_break_interval,
  });
}
export function planError(error: unknown) {
  const key = String(error);
  const messages: Record<string, () => string> = {
    plan_invalid_name: m.plan_invalid_name,
    plan_duplicate_name: m.plan_duplicate_name,
    plan_last: m.plan_last,
    plan_in_use: m.plan_in_use,
    plan_missing: m.plan_missing,
    plan_stale_round: m.plan_stale_round,
    plan_invalid_config: m.plan_invalid_config,
  };
  return (messages[key] ?? m.plan_error)();
}
