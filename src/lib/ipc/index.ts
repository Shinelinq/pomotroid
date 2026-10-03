// Typed wrappers around Tauri invoke() and listen().
// All backend communication goes through this module.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { open as dialogOpen } from '@tauri-apps/plugin-dialog';
import type {
  ImportOptions,
  ImportPreview,
  Conflict,
  Rename,
  CommitResult,
  Saved,
  ReportScope,
  ReportInfo,
  Distribution,
} from '$lib/data/types';

export const dataBegin = () => invoke<string>('data_begin');
export const dataCancel = (token: string) => invoke<void>('data_cancel', { token });
export const dataRead = (token: string, locale: string) =>
  invoke<ImportPreview | null>('data_read', { token, locale });
export const dataReplan = (token: string, options: ImportOptions) =>
  invoke<ImportPreview>('data_replan', { token, options });
export const dataDetails = (token: string, offset: number, renames: boolean) =>
  invoke<{ conflicts: Conflict[]; renames: Rename[] }>('data_details', { token, offset, renames });
export const dataCommit = (token: string) => invoke<CommitResult>('data_commit', { token });
export const dataExport = (profiles: boolean, preferences: boolean) =>
  invoke<Saved | null>('data_export', { profiles, preferences });
export const dataLastExport = () => invoke<string | null>('data_last_export');
export const reportPreview = (token: string, scope: ReportScope, locale: string) =>
  invoke<ReportInfo>('report_preview', { token, scope, locale });
export const reportSave = (token: string) => invoke<Saved | null>('report_save', { token });
export const statsDistribution = (start: string, end: string) =>
  invoke<Distribution>('stats_distribution', { start, end });
export const onDataChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('data:changed', cb);
export const dataViewStats = () => invoke<void>('data_view_stats');
export const onStatsAll = (cb: () => void): Promise<UnlistenFn> => listen<void>('stats:all', cb);
export const onDataPhase = (
  cb: (value: { token: string; phase: string }) => void
): Promise<UnlistenFn> =>
  listen<{ token: string; phase: string }>('data:phase', (e) => cb(e.payload));
import type {
  TimerState,
  SessionQuery,
  SessionPage,
  CategoryState,
  CategoryAction,
  CategoryFilter,
  PlanState,
  PlanAction,
  Settings,
  Theme,
  CustomAudioInfo,
  DetailedStats,
  HeatmapStats,
  UpdateInfo,
  MiniInfo,
} from '$lib/types';

// --- Timer commands ---

export const timerToggle = () => invoke<void>('timer_toggle');
export const timerReset = () => invoke<void>('timer_reset');
export const timerRestartRound = () => invoke<void>('timer_restart_round');
export const timerSkip = () => invoke<void>('timer_skip');
export const getTimerState = () => invoke<TimerState>('timer_get_state');

// --- Settings commands ---

export const getSettings = () => invoke<Settings>('settings_get');
/** Save a single setting key/value pair and receive the full updated settings. */
export const setSetting = (key: string, value: string) =>
  invoke<Settings>('settings_set', { key, value });
export const resetSettings = () => invoke<Settings>('settings_reset_defaults');
export const reloadShortcuts = () => invoke<void>('shortcuts_reload');

// --- Theme commands ---

export const getThemes = () => invoke<Theme[]>('themes_list');

// --- Notification commands ---

export const notificationShow = (title: string, body: string) =>
  invoke<void>('notification_show', { title, body });

// --- Window commands ---

export const setWindowVisibility = (visible: boolean) =>
  invoke<void>('window_set_visibility', { visible });

export const openAuxiliaryWindow = (kind: 'settings' | 'stats') =>
  invoke<void>('aux_window_open', { kind });
export const auxiliaryWindowReady = () =>
  invoke<void>('aux_window_ready', {
    token: Number(new URLSearchParams(window.location.search).get('aux') ?? 0),
  });

export const openMini = () => invoke<void>('mini_open');
export const getMiniInfo = (token: number) => invoke<MiniInfo>('mini_info', { token });
export const miniReady = (token: number) => invoke<void>('mini_ready', { token });
export const miniFailed = (token: number, message: string) =>
  invoke<void>('mini_failed', { token, message });
export const restoreFromMini = (token: number) => invoke<void>('mini_restore', { token });
export const hideMiniToTray = (token: number) => invoke<void>('mini_hide_to_tray', { token });
export const setMiniTop = (token: number, value: boolean) =>
  invoke<void>('mini_set_top', { token, value });
export const saveMiniPosition = (token: number) => invoke<void>('mini_save_position', { token });
export const exitFromMini = (token: number) => invoke<void>('mini_exit', { token });
export const onMiniError = (cb: () => void): Promise<UnlistenFn> => listen<void>('mini:error', cb);

// --- Audio commands ---

export const getCustomAudioInfo = () => invoke<CustomAudioInfo>('audio_get_custom_info');

/** Copy `srcPath` to the config dir for `cue`; returns the display name. */
export const setCustomAudio = (cue: string, srcPath: string) =>
  invoke<string>('audio_set_custom', { cue, srcPath });

/** Delete the custom file for `cue` and revert to the built-in sound. */
export const clearCustomAudio = (cue: string) => invoke<void>('audio_clear_custom', { cue });

/** Open a native file picker filtered to audio formats. Returns a path or null. */
export const openAudioFilePicker = (): Promise<string | null> =>
  dialogOpen({
    multiple: false,
    filters: [{ name: 'Audio', extensions: ['mp3', 'wav', 'ogg'] }],
  }) as Promise<string | null>;

// --- Diagnostic log commands ---

/** Open the application log directory in the OS file manager. */
export const openLogDir = () => invoke<void>('open_log_dir');

/** Return the resolved log directory path as a string. */
export const getLogDir = () => invoke<string>('get_log_dir');

/** Return the compile-time build version string (e.g. `1.0.0-dev.80+20b2d87`). */
export const appVersion = () => invoke<string>('app_version');

// --- Sessions commands ---

export const clearSessionHistory = () => invoke<void>('sessions_clear');

// --- Stats commands ---

/** Daily + weekly data + streak in one call (Today and This Week tabs). */
export const statsGetDetailed = (filter?: CategoryFilter) =>
  invoke<DetailedStats>('stats_get_detailed', { filter });

/** Heatmap entries + lifetime totals (All Time tab). */
export const statsGetHeatmap = (filter?: CategoryFilter) =>
  invoke<HeatmapStats>('stats_get_heatmap', { filter });

// --- Platform commands ---

export const accessibilityTrusted = () => invoke<boolean>('accessibility_trusted');

/** Returns true if the system tray is usable on this platform/install.
 *  On Linux this probes for libayatana-appindicator3 / libappindicator3 at
 *  runtime; on macOS and Windows it always returns true. */
export const traySupported = () => invoke<boolean>('tray_supported');

// --- Updater commands ---

/** Check for an available update. Returns update info or null if already up to date. */
export const checkUpdate = () => invoke<UpdateInfo | null>('check_update');

/** Download, install, and immediately relaunch with the pending update. */
export const installUpdate = () => invoke<void>('install_update');

// --- Event listeners ---

export const onTimerTick = (
  cb: (payload: { elapsed_secs: number; total_secs: number }) => void
): Promise<UnlistenFn> =>
  listen<{ elapsed_secs: number; total_secs: number }>('timer:tick', (e) => cb(e.payload));

export const onTimerPaused = (
  cb: (payload: { elapsed_secs: number }) => void
): Promise<UnlistenFn> => listen<{ elapsed_secs: number }>('timer:paused', (e) => cb(e.payload));

export const onTimerResumed = (
  cb: (payload: { elapsed_secs: number }) => void
): Promise<UnlistenFn> => listen<{ elapsed_secs: number }>('timer:resumed', (e) => cb(e.payload));

export const onRoundChange = (cb: (state: TimerState) => void): Promise<UnlistenFn> =>
  listen<TimerState>('timer:round-change', (e) => cb(e.payload));

export const onTimerReset = (cb: (state: TimerState) => void): Promise<UnlistenFn> =>
  listen<TimerState>('timer:reset', (e) => cb(e.payload));

export const onSettingsChanged = (cb: (settings: Settings) => void): Promise<UnlistenFn> =>
  listen<Settings>('settings:changed', (e) => cb(e.payload));

export const onThemesChanged = (cb: (themes: Theme[]) => void): Promise<UnlistenFn> =>
  listen<Theme[]>('themes:changed', (e) => cb(e.payload));

export const onSessionsCleared = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('sessions:cleared', () => cb());

// Timer plans and round controls share the backend's authoritative snapshots.
export const getTimerPlans = () => invoke<PlanState>('timer_plans_get');
export const timerPlanAction = (action: PlanAction) =>
  invoke<PlanState>('timer_plans_action', { action });
export const manageTimerPlans = () => invoke<void>('timer_plans_manage');
export const takeTimerPlansFocus = () => invoke<boolean>('timer_plans_take_focus');
export const onTimerPlansFocus = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('plans:focus', cb);
export const onTimerState = (cb: (state: TimerState) => void): Promise<UnlistenFn> =>
  listen<TimerState>('timer:state', (e) => cb(e.payload));
export const onTimerPlansChanged = (cb: (state: PlanState) => void): Promise<UnlistenFn> =>
  listen<PlanState>('plans:changed', (e) => cb(e.payload));

// Classification has its own events; it never reconfigures timer settings.
export const getCategories = () => invoke<CategoryState>('categories_get');
export const categoryAction = (action: CategoryAction) =>
  invoke<CategoryState>('categories_action', { action });
export const manageCategories = () => invoke<void>('categories_manage');
export const takeCategoriesFocus = () => invoke<boolean>('categories_take_focus');
export const onCategoriesFocus = (cb: () => void): Promise<UnlistenFn> =>
  listen<void>('categories:focus', cb);
export const onCategoriesChanged = (cb: (state: CategoryState) => void): Promise<UnlistenFn> =>
  listen<CategoryState>('categories:changed', (e) => cb(e.payload));

export const statsGetSessions = (query: SessionQuery) =>
  invoke<SessionPage>('stats_get_sessions', { query });

export const dragMini = (token: number) => invoke<void>('mini_drag', { token });
export const setMiniBehavior = (token: number, key: 'snap' | 'locked', value: boolean) =>
  invoke<MiniInfo>('mini_set_behavior', { token, key, value });
export const onMiniPreferences = (cb: (value: MiniInfo) => void): Promise<UnlistenFn> =>
  listen<MiniInfo>('mini:preferences', (e) => cb(e.payload));

export const onMiniStatus = (handler: (lines: string[]) => void) =>
  listen<string[]>('mini:status', (event) => handler(event.payload));
