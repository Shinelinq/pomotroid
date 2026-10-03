// Reactive timer state store.
// Populated by Tauri event listeners (timer:tick, timer:round-change, etc.).

import { writable } from 'svelte/store';
import type { TimerState } from '$lib/types';

const initial: TimerState = {
  round_type: 'work',
  previous_round_type: '',
  elapsed_secs: 0,
  total_secs: 25 * 60,
  is_running: false,
  is_paused: false,
  work_round_number: 1,
  work_rounds_total: 4,
  session_work_count: 1,
  round_id: 0,
  revision: -1,
  captured_at_ms: 0,
  has_started: false,
  stop_after_round: false,
  stopped_after_round: false,
  category_id: null,
  next_category_id: null,
  category_pending: false,
  category_notice_id: null,
  session_id: null,
};

export const timerState = writable<TimerState>(initial);
