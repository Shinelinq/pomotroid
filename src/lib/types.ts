// Shared TypeScript types mirroring Rust structs (must stay in sync with Rust serde output).

export type RoundType = 'work' | 'short-break' | 'long-break';

/** Mirrors Rust `TimerSnapshot` — emitted via timer:tick / timer:round-change events
 *  and returned by the `timer_get_state` IPC command. */
export interface TimerState {
  round_type: RoundType;
  previous_round_type: string; // round type before this one; "" on first round
  elapsed_secs: number;
  total_secs: number;
  is_running: boolean;
  is_paused: boolean;
  work_round_number: number; // current work round (1-based)
  work_rounds_total: number; // total work rounds before long break
  session_work_count: number; // monotonic focus round count since last reset
  round_id: number;
  revision: number;
  captured_at_ms: number;
  has_started: boolean;
  stop_after_round: boolean;
  stopped_after_round: boolean;
  category_id: number | null;
  next_category_id: number | null;
  category_pending: boolean;
  category_notice_id: number | null;
  session_id: number | null;
}

/** Mirrors Rust `Settings` struct returned by `settings_get`. */
export interface Settings {
  time_work_secs: number;
  time_short_break_secs: number;
  time_long_break_secs: number;
  long_break_interval: number;
  short_breaks_enabled: boolean;
  long_breaks_enabled: boolean;
  auto_start_work: boolean;
  auto_start_break: boolean;
  tray_icon_enabled: boolean;
  tray_display_mode: 'progress' | 'minutes';
  min_to_tray: boolean;
  min_to_tray_on_close: boolean;
  notifications_enabled: boolean;
  always_on_top: boolean;
  break_always_on_top: boolean;
  volume: number; // 0.0–1.0
  tick_sounds_during_work: boolean;
  tick_sounds_during_break: boolean;
  shortcut_toggle: string;
  shortcut_reset: string;
  shortcut_skip: string;
  shortcut_restart: string;
  websocket_enabled: boolean;
  websocket_port: number;
  theme_mode: string; // 'auto' | 'light' | 'dark'
  theme_light: string;
  theme_dark: string;
  dial_countdown: boolean;
  language: string; // 'auto' | 'en' | 'es' | 'fr' | 'de' | 'ja'
  verbose_logging: boolean;
  check_for_updates: boolean;
  global_shortcuts_enabled: boolean;
  local_shortcut_toggle: string;
  local_shortcut_reset: string;
  local_shortcut_skip: string;
  local_shortcut_volume_down: string;
  local_shortcut_volume_up: string;
  local_shortcut_mute: string;
  local_shortcut_fullscreen: string;
}

/** Returned by `check_update` — describes an available update. */
export interface UpdateInfo {
  version: string;
  body: string | null;
  date: string | null;
}

/** Mirrors Rust `CustomAudioInfo` — null means the built-in sound is active. */
export interface CustomAudioInfo {
  work_alert: string | null;
  short_break_alert: string | null;
  long_break_alert: string | null;
}

/** Mirrors Rust `Theme` struct. Color keys include the `--` CSS var prefix. */
export interface Theme {
  name: string;
  colors: Record<string, string>; // keys like "--color-background", "--color-focus-round"
  is_custom: boolean;
}

// ---------------------------------------------------------------------------
// Stats types — mirror Rust structs in commands.rs / queries.rs
// ---------------------------------------------------------------------------

export interface DailyStats {
  rounds: number;
  focus_mins: number;
  completion_rate: number | null; // null when no sessions started today
  by_hour: number[]; // 24 entries, index = hour of day
  by_hour_focus_secs: number[]; // 24 exact second totals, attributed by local start hour
}

export interface DayStat {
  date: string; // "YYYY-MM-DD", local calendar date
  rounds: number; // completed work sessions
  started_rounds: number; // all recorded work sessions, including incomplete ones
  focus_secs: number; // exact seconds from completed work sessions
}

export interface HeatmapEntry {
  date: string; // "YYYY-MM-DD", local calendar date
  count: number; // completed work sessions
  focus_secs: number; // exact seconds from completed work sessions
}

export interface StreakInfo {
  current: number;
  longest: number;
}

/** Returned by stats_get_detailed — Today + This Week + streak in one call. */
export interface DetailedStats {
  today: DailyStats;
  week: DayStat[];
  streak: StreakInfo;
}

/** Returned by stats_get_heatmap — heatmap entries + lifetime totals. */
export interface HeatmapStats {
  entries: HeatmapEntry[];
  total_rounds: number;
  total_hours: number;
  total_focus_secs: number; // exact seconds from all completed work sessions
  longest_streak: number;
}

/** Window preferences and actual tray availability, separate from main settings. */
export interface MiniInfo {
  always_on_top: boolean;
  tray_available: boolean;
  snap_enabled: boolean;
  position_locked: boolean;
  menu_lines: string[];
}

export type TimerConfig = Pick<
  Settings,
  | 'time_work_secs'
  | 'time_short_break_secs'
  | 'time_long_break_secs'
  | 'long_break_interval'
  | 'short_breaks_enabled'
  | 'long_breaks_enabled'
  | 'auto_start_work'
  | 'auto_start_break'
>;
export interface TimerPlan {
  id: string;
  name: string;
  initial_name: boolean;
  config: TimerConfig;
}
export interface PlanState {
  book: { plans: TimerPlan[]; selected_id: string; working: TimerConfig };
  active_id: string;
  pending_id: string | null;
  pending_revision: number;
  modified: boolean;
  timer: TimerState;
}
export type PlanAction =
  | { kind: 'select'; id: string }
  | { kind: 'edit'; key: string; value: string }
  | { kind: 'save' }
  | { kind: 'save_as'; name: string }
  | { kind: 'rename'; id: string; name: string }
  | { kind: 'delete'; id: string }
  | { kind: 'template'; name: string; long: boolean }
  | { kind: 'cancel_pending' }
  | { kind: 'apply_now'; round_id: number; pending_revision: number }
  | { kind: 'stop_after'; round_id: number; enabled: boolean };

export interface Category {
  id: number;
  name: string;
  archived: boolean;
}
export interface CategoryState {
  data: { items: Category[]; selected_id: number | null };
  revision: number;
  timer: TimerState;
}
export type CategoryFilter =
  | { kind: 'all' }
  | { kind: 'uncategorized' }
  | { kind: 'category'; category_id: number };
export type CategoryAction =
  | { kind: 'create'; name: string }
  | { kind: 'rename'; id: number; name: string }
  | { kind: 'archive'; id: number }
  | { kind: 'restore'; id: number }
  | { kind: 'select'; id: number | null }
  | { kind: 'cancel_pending'; round_id: number }
  | { kind: 'dismiss_notice' };

export interface SessionRecord {
  id: number;
  started_at: number;
  ended_at: number | null;
  duration_secs: number;
  completed: boolean;
  category_id: number | null;
  category_name: string | null;
  category_archived: boolean;
}
export interface SessionSummary {
  recorded: number;
  completed: number;
  focus_secs: number;
}
export interface SessionCursor {
  date: string;
  hour: number | null;
  filter: CategoryFilter;
  started_at: number;
  id: number;
  max_id: number;
}
export interface SessionQuery {
  date: string;
  hour?: number | null;
  filter: CategoryFilter;
  cursor?: SessionCursor | null;
  limit?: number;
}
export interface SessionPage {
  records: SessionRecord[];
  summary: SessionSummary;
  next_cursor: SessionCursor | null;
}
