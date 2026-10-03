import type { SessionRecord, TimerState } from '../../types';

export interface ChartSelection {
  pinned: string | null;
  focused: string | null;
}
export interface DetailEntry {
  date: string;
  hour?: number | null;
  focusKey: string;
}

export function calendarDate(key: string) {
  const [year, month, day] = key.split('-').map(Number);
  const date = new Date(0);
  date.setUTCHours(12, 0, 0, 0);
  date.setUTCFullYear(year, month - 1, day);
  return date;
}
export function shiftDetailDate(key: string, days: number): string | null {
  const date = calendarDate(key);
  date.setUTCDate(date.getUTCDate() + days);
  const year = date.getUTCFullYear();
  if (year < 1 || year > 9999) return null;
  return `${String(year).padStart(4, '0')}-${String(date.getUTCMonth() + 1).padStart(2, '0')}-${String(date.getUTCDate()).padStart(2, '0')}`;
}
export function localDayKey(date: Date) {
  return `${String(date.getFullYear()).padStart(4, '0')}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
export function exactDuration(seconds: number) {
  const hours = Math.floor(seconds / 3600),
    minutes = Math.floor((seconds % 3600) / 60),
    remainder = seconds % 60;
  return (
    [hours ? `${hours}h` : '', minutes ? `${minutes}m` : '', remainder ? `${remainder}s` : '']
      .filter(Boolean)
      .join(' ') || '0m'
  );
}
export function sessionStatus(record: SessionRecord, runtime: TimerState | null) {
  if (record.completed) return 'completed' as const;
  if (runtime?.round_type === 'work' && runtime.has_started && runtime.session_id === record.id) {
    if (runtime.is_running) return 'running' as const;
    if (runtime.is_paused) return 'paused' as const;
  }
  return 'incomplete' as const;
}
/** End times are never synthesized from planned duration. */
export function sessionTimes(record: SessionRecord, locale: string) {
  const start = new Date(record.started_at * 1000);
  const time = new Intl.DateTimeFormat(locale, {
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  });
  const full = new Intl.DateTimeFormat(locale, {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
    hourCycle: 'h23',
  });
  if (record.ended_at === null)
    return {
      start: time.format(start),
      end: null,
      endDate: null,
      nextDay: false,
      title: full.format(start),
    };
  const end = new Date(record.ended_at * 1000);
  const startDay = localDayKey(start),
    endDay = localDayKey(end);
  return {
    start: time.format(start),
    end: time.format(end),
    nextDay: shiftDetailDate(startDay, 1) === endDay,
    endDate:
      startDay === endDay
        ? null
        : new Intl.DateTimeFormat(locale, {
            year: 'numeric',
            month: 'numeric',
            day: 'numeric',
          }).format(end),
    title: `${full.format(start)} — ${full.format(end)}`,
  };
}
