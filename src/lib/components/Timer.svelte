<script lang="ts">
  // Orchestrator component. Subscribes to authoritative timer state,
  // and renders TimerDial + TimerDisplay + TimerFooter.
  import { onMount } from 'svelte';
  import { timerToggle, timerRestartRound, timerSkip, onRoundChange } from '$lib/ipc';
  import { connectPlans } from '$lib/plans/state';
  import RoundStatus from './plans/RoundStatus.svelte';
  import { timerState } from '$lib/stores/timer';
  import { settings } from '$lib/stores/settings';
  import { fade } from 'svelte/transition';
  import TimerDial from './TimerDial.svelte';
  import TimerDisplay from './TimerDisplay.svelte';
  import TimerFooter from './TimerFooter.svelte';
  import MiniControls from './MiniControls.svelte';
  import Tooltip from './Tooltip.svelte';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import * as m from '$paraglide/messages.js';
  import { notificationShow } from '$lib/ipc';
  import { timerLayoutStyle, type MainTimerLayout } from '$lib/utils/mainTimerLayout';

  interface Props {
    layout: MainTimerLayout;
    extraStatusRows?: number;
  }

  let { layout, extraStatusRows = $bindable(0) }: Props = $props();
  const isCompact = $derived(layout.compact);
  const layoutStyle = $derived(timerLayoutStyle(layout));

  let state = $derived($timerState);

  function roundColor(rt: string): string {
    if (rt === 'work') return 'var(--color-focus-round)';
    if (rt === 'short-break') return 'var(--color-short-round)';
    return 'var(--color-long-round)';
  }

  function roundLabel(rt: string): string {
    if (rt === 'work') return m.round_label_work();
    if (rt === 'short-break') return m.round_label_short_break();
    return m.round_label_long_break();
  }

  onMount(() => {
    const connection = connectPlans();
    let disposed = false;
    let stop: UnlistenFn | undefined;
    onRoundChange((snap) => {
      if (disposed || !$settings.notifications_enabled) return;
      let title: string;
      let body: string;
      if (snap.stopped_after_round) {
        title = m.round_stopped_title();
        body = m.round_stopped_body();
      } else if (snap.round_type === 'work') {
        const afterBreak =
          snap.previous_round_type === 'short-break' || snap.previous_round_type === 'long-break';
        title = afterBreak ? m.notification_work_title() : m.notification_work_start_title();
        body = afterBreak ? m.notification_work_body() : m.notification_work_start_body();
      } else if (snap.round_type === 'short-break') {
        title = m.notification_short_break_title();
        body = m.notification_short_break_body();
      } else {
        title = m.notification_long_break_title();
        body = m.notification_long_break_body();
      }
      void notificationShow(title, body).catch(() => {});
    }).then((unlisten) => {
      if (disposed) unlisten();
      else stop = unlisten;
    });
    return () => {
      disposed = true;
      connection.dispose();
      stop?.();
    };
  });
</script>

<div class="timer-outer" class:compact={isCompact} style={layoutStyle}>
  <div class="timer">
    <!-- Dial + display stacked (display centered over dial) -->
    <div class="dial-stack">
      <TimerDial snap={state} countdown={$settings.dial_countdown} mainWindow />
      <TimerDisplay {state} mainWindow />
    </div>

    <div class="regular">
      <!-- Round type label sits below the dial as a normal flex child so it
           does not affect the dial-stack height used to centre TimerDisplay. -->
      <div class="round-label" style="color: {roundColor(state.round_type)}">
        {roundLabel(state.round_type)}
      </div>

      <RoundStatus onExtraRowsChange={(rows) => (extraStatusRows = rows)} />
      <div class="controls-wrapper">
        <!-- Back: restart current round -->
        <Tooltip text={m.tooltip_restart_round()} followLayout>
          <button class="btn-side" onclick={timerRestartRound} aria-label="Restart round">
            <svg width="18" height="18" viewBox="0 0 16 16">
              <polygon points="15,1 6,8 15,15" fill="currentColor" />
              <rect x="1" y="1" width="3" height="14" rx="1" fill="currentColor" />
            </svg>
          </button>
        </Tooltip>

        <!-- Play / Pause — icon fades when state changes -->
        <button
          class="play-pause"
          onclick={timerToggle}
          aria-label={state.is_running ? 'Pause' : 'Play'}
        >
          {#key state.is_running}
            <span class="icon" in:fade={{ duration: 120 }}>
              {#if state.is_running}
                <svg viewBox="0 0 24 24">
                  <rect x="5" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                  <rect x="14" y="3" width="5" height="18" rx="1.5" fill="currentColor" />
                </svg>
              {:else}
                <svg viewBox="0 0 24 24">
                  <polygon points="5,2 23,12 5,22" fill="currentColor" />
                </svg>
              {/if}
            </span>
          {/key}
        </button>

        <!-- Skip: advance to next round -->
        <Tooltip text={m.tooltip_skip()} followLayout>
          <button class="btn-side" onclick={timerSkip} aria-label="Skip round">
            <svg width="18" height="18" viewBox="0 0 16 16">
              <polygon points="1,1 10,8 1,15" fill="currentColor" />
              <rect x="12" y="1" width="3" height="14" rx="1" fill="currentColor" />
            </svg>
          </button>
        </Tooltip>

        <TimerFooter snap={state} mainWindow />
      </div>
    </div>
  </div>

  {#if isCompact}
    <MiniControls />
  {/if}
</div>

<style>
  .timer-outer {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
    flex: none;
  }

  .timer {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
  }

  .regular {
    display: contents;
  }

  .compact .regular {
    display: none;
  }

  .dial-stack {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .controls-wrapper {
    display: grid;
    width: var(--main-timer-controls);
    grid-template-columns: repeat(3, minmax(0, 1fr));
    grid-template-rows: var(--main-timer-play) var(--main-timer-footerHeight);
    align-items: center;
    justify-items: center;
    row-gap: var(--main-timer-controlGap);
    margin-top: var(--main-timer-controlGap);
  }
  .controls-wrapper > :global(*) {
    min-width: 0;
  }

  .btn-side {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--color-foreground-darker, var(--color-foreground));
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--main-timer-side);
    height: var(--main-timer-side);
    border-radius: 4px;
    transition:
      color var(--transition-default),
      background var(--transition-default);
  }

  .btn-side:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .play-pause {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--color-foreground);
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--main-timer-play);
    height: var(--main-timer-play);
    border-radius: 50%;
    border: var(--main-timer-border) solid var(--color-foreground-darker, var(--color-foreground));
    transition:
      color var(--transition-default),
      border-color var(--transition-default),
      background var(--transition-default);
    overflow: hidden; /* clip the fading icon within the circle */
  }

  .play-pause:hover {
    color: var(--color-accent);
    border-color: var(--color-accent);
    background: var(--color-hover);
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon svg {
    width: var(--main-timer-playIcon);
    height: var(--main-timer-playIcon);
  }

  .btn-side svg {
    width: var(--main-timer-sideIcon);
    height: var(--main-timer-sideIcon);
  }

  .controls-wrapper button:focus-visible {
    outline: 2px solid var(--color-foreground);
    outline-offset: 3px;
  }

  .compact :global(.mini-controls button) {
    width: 32px;
    height: 32px;
  }

  .compact :global(.mini-controls svg) {
    width: 14px;
    height: 14px;
  }

  .round-label {
    font-size: var(--main-timer-labelFont);
    line-height: var(--main-timer-labelHeight);
    height: var(--main-timer-labelHeight);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    margin-top: var(--main-timer-labelGap);
    margin-bottom: var(--main-timer-statusGap);
  }
</style>
