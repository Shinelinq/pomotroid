import { onDestroy } from 'svelte';

/** Window-local chart interaction; never persists or changes timer state. */
export function createChartInteraction(initialPinned: string | null = null) {
  let preview = $state<{ date: string; anchor: Element } | null>(null);
  let pinned = $state<string | null>(initialPinned);
  let showTimer: ReturnType<typeof setTimeout> | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  function cancelTimers() {
    clearTimeout(showTimer);
    clearTimeout(hideTimer);
  }
  function show(date: string, anchor: Element, immediate = false) {
    cancelTimers();
    if (immediate) preview = { date, anchor };
    else
      showTimer = setTimeout(() => {
        preview = { date, anchor };
      }, 150);
  }
  function leave() {
    cancelTimers();
    hideTimer = setTimeout(() => {
      preview = null;
    }, 180);
  }
  function clear() {
    cancelTimers();
    preview = null;
    pinned = null;
  }
  onDestroy(cancelTimers);
  return {
    get preview() {
      return preview;
    },
    get pinned() {
      return pinned;
    },
    show,
    leave,
    clear,
    keep: cancelTimers,
    hidePreview() {
      cancelTimers();
      preview = null;
    },
    toggle(date: string) {
      pinned = pinned === date ? null : date;
    },
    unpin() {
      pinned = null;
    },
    escape(event: KeyboardEvent) {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      clear();
    },
  };
}
