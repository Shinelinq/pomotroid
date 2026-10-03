<script lang="ts">
  import '../../app.css';
  import { onMount, tick } from 'svelte';
  import { error as logError } from '@tauri-apps/plugin-log';
  import MiniTimer from '$lib/components/MiniTimer.svelte';
  import {
    getSettings,
    getThemes,
    onSettingsChanged,
    onThemesChanged,
    getTimerState,
    onTimerTick,
    onTimerState,
    onTimerPaused,
    onTimerResumed,
    onRoundChange,
    onTimerReset,
    timerToggle,
    timerRestartRound,
    timerSkip,
    getMiniInfo,
    miniReady,
    miniFailed,
    restoreFromMini,
    hideMiniToTray,
    setMiniTop,
    dragMini,
    setMiniBehavior,
    onMiniPreferences,
    exitFromMini,
  } from '$lib/ipc';
  import { syncMiniTimer } from '$lib/mini/sync';
  import { popupMiniMenu } from '$lib/mini/menu';
  import { setLocale } from '$lib/locale.svelte.js';
  import { resolveThemeName } from '$lib/utils/theme';
  import { applyTheme } from '$lib/stores/theme';
  import type { MiniInfo, TimerState, Settings, Theme } from '$lib/types';

  const token = Number(new URLSearchParams(window.location.search).get('instance'));
  let timerSnapshot = $state<TimerState | null>(null);
  let smooth = $state(false);
  let busy = $state(false);
  let failed = $state(false);
  let disposed = false;
  let menuOpen = false;
  let menuClosedAt = 0;
  let miniInfo = $state<MiniInfo | null>(null);
  let saved: Settings | null = null;
  let themes: Theme[] = [];
  let settingsRevision = 0;
  let themesRevision = 0;
  let stopSync: (() => void) | undefined;

  const report = (error: unknown) => logError(`[mini] ${error}`).catch(() => {});
  function applyAppearance() {
    if (!saved) return;
    const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const theme = themes.find((item) => item.name === resolveThemeName(saved!, dark)) ?? themes[0];
    if (theme) applyTheme(theme);
  }
  async function timerAction(action: () => Promise<void>) {
    if (disposed || busy || failed || !timerSnapshot) return;
    busy = true;
    try {
      await action();
    } catch (error) {
      if (!disposed) {
        failed = true;
        timerSnapshot = null;
        stopSync?.();
      }
      void report(error);
    } finally {
      if (!disposed) busy = false;
    }
  }
  async function windowAction(action: () => Promise<void>) {
    if (disposed) return;
    try {
      await action();
    } catch (error) {
      if (!disposed) failed = true;
      void report(error);
    }
  }
  const restore = () => windowAction(() => restoreFromMini(token));
  async function drag() {
    if (disposed || menuOpen || miniInfo?.position_locked) return;
    await windowAction(async () => {
      await dragMini(token);
    });
  }
  async function menu() {
    if (menuOpen || disposed) return;
    menuOpen = true;
    try {
      const info = await getMiniInfo(token);
      miniInfo = info;
      if (disposed) return;
      await popupMiniMenu(failed || busy ? null : timerSnapshot, info, {
        toggle: () => void timerAction(timerToggle),
        restart: () => void timerAction(timerRestartRound),
        skip: () => void timerAction(timerSkip),
        top: () => void windowAction(() => setMiniTop(token, !info.always_on_top)),
        snap: () =>
          void windowAction(async () => {
            miniInfo = await setMiniBehavior(token, 'snap', !info.snap_enabled);
          }),
        lock: () =>
          void windowAction(async () => {
            miniInfo = await setMiniBehavior(token, 'locked', !info.position_locked);
          }),
        restore: () => void restore(),
        hide: () => void windowAction(() => hideMiniToTray(token)),
        exit: () => void windowAction(() => exitFromMini(token)),
      });
    } catch (error) {
      void report(error);
    } finally {
      menuOpen = false;
      menuClosedAt = performance.now();
    }
  }
  function keydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    event.preventDefault();
    event.stopPropagation();
    if (!menuOpen && performance.now() - menuClosedAt > 250 && !event.repeat) void restore();
  }

  onMount(() => {
    const cleanups: (() => void)[] = [];
    async function keep(registration: Promise<() => void>) {
      const unlisten = await registration;
      if (disposed) unlisten();
      else cleanups.push(unlisten);
    }
    let sync: ReturnType<typeof syncMiniTimer> | undefined;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', applyAppearance);
    void (async () => {
      try {
        if (!Number.isSafeInteger(token) || token <= 0) throw new Error('invalid mini instance');
        await keep(
          onMiniPreferences((value) => {
            miniInfo = value;
          })
        );
        miniInfo = await getMiniInfo(token);
        // Observe settings and theme changes before reading their initial snapshots.
        await Promise.all([
          keep(
            onSettingsChanged((value) => {
              ++settingsRevision;
              saved = value;
              setLocale(value.language);
              applyAppearance();
            })
          ),
          keep(
            onThemesChanged((value) => {
              ++themesRevision;
              themes = value;
              applyAppearance();
            })
          ),
        ]);
        const settingsVersion = settingsRevision;
        const themesVersion = themesRevision;
        const [initial, initialThemes] = await Promise.all([getSettings(), getThemes()]);
        if (disposed) return;
        if (settingsVersion === settingsRevision) saved = initial;
        if (themesVersion === themesRevision) themes = initialThemes;
        setLocale(saved!.language);
        applyAppearance();
        await Promise.all([
          document.fonts.load('300 24px "Mona Sans Mono"'),
          document.fonts.load('400 10px "Mona Sans"'),
        ]);
        await document.fonts.ready;
        if (disposed) return;
        sync = syncMiniTimer(
          {
            getTimerState,
            onTimerState,
            onTimerTick,
            onTimerPaused,
            onTimerResumed,
            onRoundChange,
            onTimerReset,
          },
          (snapshot, animate) => {
            if (!disposed) {
              timerSnapshot = snapshot;
              smooth = animate;
            }
          }
        );
        stopSync = sync.dispose;
        await sync.ready;
        await tick();
        if (!disposed) await miniReady(token);
      } catch (error) {
        sync?.dispose();
        if (!disposed) {
          failed = true;
          timerSnapshot = null;
          await miniFailed(token, String(error)).catch(report);
        }
      }
    })();
    return () => {
      disposed = true;
      sync?.dispose();
      for (const cleanup of cleanups) cleanup();
      mq.removeEventListener('change', applyAppearance);
    };
  });
</script>

<svelte:window onkeydown={keydown} />
<MiniTimer
  {timerSnapshot}
  locked={miniInfo?.position_locked ?? false}
  {smooth}
  {busy}
  error={failed}
  onToggle={() => void timerAction(timerToggle)}
  onSkip={() => void timerAction(timerSkip)}
  onRestore={() => void restore()}
  onDrag={drag}
  onMenu={() => void menu()}
/>
