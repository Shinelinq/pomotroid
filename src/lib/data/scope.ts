import { dateKey, shiftDate } from '../components/stats/stats';
import type { ReportScope } from './types';

export function reportDates(
  preset: string,
  now = new Date()
): { start: string | null; end: string | null } {
  const today = dateKey(now);
  if (preset === 'all') return { start: null, end: null };
  if (preset === 'month')
    return {
      start: dateKey(new Date(now.getFullYear(), now.getMonth(), 1)),
      end: dateKey(new Date(now.getFullYear(), now.getMonth() + 1, 0)),
    };
  if (preset === 'previous')
    return {
      start: dateKey(new Date(now.getFullYear(), now.getMonth() - 1, 1)),
      end: dateKey(new Date(now.getFullYear(), now.getMonth(), 0)),
    };
  return { start: shiftDate(today, -6), end: today };
}
export function overviewScope(tab: string, today: string, year: number): ReportScope {
  return {
    kind: 'daily',
    start: tab === 'today' ? today : tab === 'week' ? shiftDate(today, -6) : `${year}-01-01`,
    end: tab === 'alltime' ? `${year}-12-31` : today,
    filter: { kind: 'all' },
    hour: null,
  };
}
export function percentage(seconds: number, total: number): string {
  if (total <= 0 || seconds <= 0) return '0%';
  const value = (seconds * 100) / total;
  return value < 0.1 ? '<0.1%' : `${Number(value.toFixed(1))}%`;
}
