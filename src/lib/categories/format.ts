import * as m from '$paraglide/messages.js';
import type { Category, CategoryFilter, TimerState } from '$lib/types';

export function categoryName(
  items: Category[],
  id: number | null | undefined,
  markArchived = true
) {
  if (id == null) return m.category_uncategorized();
  const category = items.find((item) => item.id === id);
  if (!category) return m.category_unknown();
  return category.archived && markArchived
    ? m.category_archived_name({ name: category.name })
    : category.name;
}
export function timerCategoryLabel(items: Category[], timer: TimerState) {
  if (timer.round_type !== 'work')
    return m.category_next_focus({ name: categoryName(items, timer.next_category_id) });
  return categoryName(items, timer.has_started ? timer.category_id : timer.next_category_id);
}
export const filterKey = (filter: CategoryFilter) =>
  filter.kind === 'category' ? `category:${filter.category_id}` : filter.kind;
export function filterName(items: Category[], filter: CategoryFilter) {
  return filter.kind === 'all'
    ? m.category_all()
    : categoryName(items, filter.kind === 'uncategorized' ? null : filter.category_id);
}
export function categoryError(error: unknown) {
  const messages: Record<string, () => string> = {
    category_invalid_name: m.category_invalid_name,
    category_duplicate_name: m.category_duplicate_name,
    category_reserved_name: m.category_reserved_name,
    category_invalid_id: m.category_invalid_id,
    category_missing: m.category_missing,
    category_archived_error: m.category_archived_error,
    category_invalid_action: m.category_invalid_action,
    category_stale_round: m.category_stale_round,
  };
  return (messages[String(error)] ?? m.category_error)();
}
