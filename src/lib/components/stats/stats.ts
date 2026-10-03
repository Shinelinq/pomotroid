import type { DayStat, HeatmapEntry } from '$lib/types';

export type Metric = 'time' | 'rounds';

export function formatDuration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const remainder = minutes % 60;
  return hours === 0 ? `${minutes}m` : remainder === 0 ? `${hours}h` : `${hours}h ${remainder}m`;
}

export function formatRate(rounds: number, started: number): string {
  return started === 0 ? '—' : `${Math.round((rounds / started) * 100)}%`;
}

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}

export function localDate(key: string): Date {
  const [year, month, day] = key.split('-').map(Number);
  return new Date(year, month - 1, day);
}

export function shiftDate(key: string, days: number): string {
  const date = localDate(key);
  date.setDate(date.getDate() + days);
  return dateKey(date);
}

export function buildWeek(entries: DayStat[], today: string): DayStat[] {
  const byDate = new Map(entries.map((entry) => [entry.date, entry]));
  return Array.from({ length: 7 }, (_, index) => {
    const date = shiftDate(today, index - 6);
    // Only absent dates are zero-filled. Existing entries retain the complete backend contract.
    return byDate.get(date) ?? { date, rounds: 0, started_rounds: 0, focus_secs: 0 };
  });
}

export function summarizeWeek(days: DayStat[]) {
  const rounds = days.reduce((sum, day) => sum + day.rounds, 0);
  const started = days.reduce((sum, day) => sum + day.started_rounds, 0);
  const seconds = days.reduce((sum, day) => sum + day.focus_secs, 0);
  const active = days.filter((day) => day.rounds > 0).length;
  return { rounds, started, seconds, active, average: active === 0 ? 0 : seconds / active };
}

export function summarizeYear(entries: HeatmapEntry[], year: number) {
  const days = entries.filter((entry) => Number(entry.date.slice(0, 4)) === year);
  let best: HeatmapEntry | null = null;
  for (const day of days) {
    if (
      day.count > 0 &&
      (!best ||
        day.focus_secs > best.focus_secs ||
        (day.focus_secs === best.focus_secs && day.date < best.date))
    )
      best = day;
  }
  return {
    seconds: days.reduce((sum, day) => sum + day.focus_secs, 0),
    rounds: days.reduce((sum, day) => sum + day.count, 0),
    active: days.filter((day) => day.count > 0).length,
    best,
  };
}

export function heatLevel(value: number, metric: Metric): 0 | 1 | 2 | 3 {
  if (value === 0) return 0;
  const [low, middle] = metric === 'time' ? [3600, 10800] : [3, 7];
  return value <= low ? 1 : value <= middle ? 2 : 3;
}

export interface CalendarCell extends HeatmapEntry {
  week: number;
  weekday: number;
  valid: boolean;
}

export function buildYear(entries: HeatmapEntry[], year: number, today: string) {
  const first = new Date(year, 0, 1);
  const last = new Date(year, 11, 31);
  const start = new Date(first);
  start.setDate(1 - first.getDay());
  const end = new Date(last);
  end.setDate(last.getDate() + 6 - last.getDay());
  const byDate = new Map(entries.map((entry) => [entry.date, entry]));
  const cells: CalendarCell[] = [];
  const months: { month: number; week: number }[] = [];
  const cursor = new Date(start);
  for (let index = 0; cursor <= end; index++, cursor.setDate(cursor.getDate() + 1)) {
    const date = dateKey(cursor);
    const week = Math.floor(index / 7);
    const entry = byDate.get(date) ?? { date, count: 0, focus_secs: 0 };
    const inYear = cursor.getFullYear() === year;
    cells.push({ ...entry, week, weekday: cursor.getDay(), valid: inYear && date <= today });
    if (inYear && cursor.getDate() === 1) months.push({ month: cursor.getMonth(), week });
  }
  return { cells, months, weeks: cells.length / 7 };
}

export function moveDate(date: string, offset: number, validDates: ReadonlySet<string>): string {
  const next = shiftDate(date, offset);
  return validDates.has(next) ? next : date;
}

/** Choose the interval first, then the ceiling: never divide an arbitrary ceiling into thirds. */
export function chartScale(maximum: number, metric: Metric, height = 240) {
  if (maximum === 0)
    return metric === 'time'
      ? { maximum: 3600, ticks: [900, 1800, 2700, 3600] }
      : { maximum: 3, ticks: [1, 2, 3] };
  const desired = Math.max(3, Math.min(5, Math.round(height / 60)));
  const steps =
    metric === 'time'
      ? [
          60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 14400, 21600, 28800, 43200, 86400,
          172800, 259200, 345600, 432000, 604800, 864000,
        ]
      : [1, 2, 3, 4, 5, 10, 20, 25, 50, 100];
  while (steps.at(-1)! * 3 < maximum * 1.02) steps.push(steps.at(-1)! * 2);
  let best = { maximum: 0, step: 1, count: 3, score: Infinity };
  for (const step of steps) {
    const count = Math.max(
      metric === 'rounds' && maximum < 2 ? 2 : 3,
      Math.ceil((maximum * 1.02) / step)
    );
    if (count > 5) continue;
    const ceiling = count * step;
    const score = (ceiling - maximum) / maximum + Math.abs(count - desired) * 0.05;
    if (score < best.score) best = { maximum: ceiling, step, count, score };
  }
  return {
    maximum: best.maximum,
    ticks: Array.from({ length: best.count }, (_, i) => (i + 1) * best.step),
  };
}

export function hourRange(hour: number): { start: string; end: string } {
  return {
    start: `${String(hour).padStart(2, '0')}:00`,
    end: `${String(hour + 1).padStart(2, '0')}:00`,
  };
}
