import * as m from '$paraglide/messages.js';

export function dataError(error: unknown): string {
  const errors: Record<string, () => string> = {
    data_format: m.data_format,
    data_version: m.data_version,
    data_corrupt: m.data_corrupt,
    data_invalid: m.data_invalid,
    data_too_large: m.data_too_large,
    data_reference: m.data_reference,
    data_internal_conflict: m.data_internal_conflict,
    data_preferences_invalid: m.data_preferences_invalid,
    data_expired: m.data_expired,
    data_read_failed: m.data_read_failed,
    data_save_failed: m.data_save_failed,
    data_protected_path: m.data_protected_path,
    data_suffix: m.data_suffix,
    data_active_round: m.data_active_round,
    data_rolled_back: m.data_rolled_back,
    report_invalid_scope: m.report_invalid_scope,
  };
  return (errors[String(error)] ?? m.data_error)();
}
export { reportDates, overviewScope, percentage } from './scope';
