<script lang="ts">
  import type { TimerState } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { miniDisplay } from '$lib/mini/sync';
  import { miniPointer } from '$lib/mini/pointer';

  let {
    timerSnapshot,
    smooth = false,
    locked = false,
    busy = false,
    error = false,
    onToggle,
    onSkip,
    onRestore,
    onDrag,
    onMenu,
  }: {
    timerSnapshot: TimerState | null;
    smooth?: boolean;
    locked?: boolean;
    busy?: boolean;
    error?: boolean;
    onToggle: () => void;
    onSkip: () => void;
    onRestore: () => void;
    onDrag: () => Promise<void>;
    onMenu: () => void;
  } = $props();
  let hovered = $state(false);
  let focused = $state(false);
  let pointerHint = $state<'toggle' | 'skip' | 'restore' | null>(null);
  let focusHint = $state<'toggle' | 'skip' | 'restore' | null>(null);
  const engaged = $derived(hovered || focused);
  const display = $derived(timerSnapshot ? miniDisplay(timerSnapshot) : null);
  const color = $derived(
    timerSnapshot?.round_type === 'short-break'
      ? 'var(--color-short-round)'
      : timerSnapshot?.round_type === 'long-break'
        ? 'var(--color-long-round)'
        : 'var(--color-focus-round)'
  );
  const toggleLabel = $derived(
    timerSnapshot?.is_running
      ? m.mini_pause()
      : timerSnapshot?.is_paused
        ? m.mini_resume()
        : m.mini_start()
  );
  const hint = $derived(pointerHint ?? focusHint);
  const hintText = $derived(
    hint === 'toggle'
      ? toggleLabel
      : hint === 'skip'
        ? m.mini_skip()
        : hint === 'restore'
          ? m.mini_expand()
          : timerSnapshot?.round_type === 'short-break'
            ? m.round_label_short_break()
            : timerSnapshot?.round_type === 'long-break'
              ? m.round_label_long_break()
              : timerSnapshot
                ? m.timer_session_round({ n: timerSnapshot.work_round_number })
                : ''
  );
  const circumference = 2 * Math.PI * 46;
</script>

<div
  class="mini"
  class:engaged
  class:locked
  role="group"
  aria-label={m.mini_title()}
  use:miniPointer={{ locked: () => locked, drag: onDrag, restore: onRestore, menu: onMenu }}
  onpointerenter={() => (hovered = true)}
  onpointerleave={() => {
    hovered = false;
    pointerHint = null;
  }}
  onfocusin={() => (focused = true)}
  onfocusout={(event) => {
    if (
      !(event.relatedTarget instanceof Node) ||
      !event.currentTarget.contains(event.relatedTarget)
    )
      focused = false;
  }}
>
  <svg class="ring" width="112" height="112" viewBox="0 0 112 112" aria-hidden="true">
    <circle
      cx="56"
      cy="56"
      r="46"
      fill="none"
      stroke="var(--color-background-light)"
      stroke-width="1"
    />
    <circle
      class="progress"
      class:smooth
      class:paused={timerSnapshot?.is_paused}
      cx="56"
      cy="56"
      r="46"
      transform="rotate(-90 56 56)"
      fill="none"
      stroke={color}
      stroke-width="4"
      stroke-linecap="round"
      stroke-dasharray={circumference}
      stroke-dashoffset={circumference * (1 - (display?.ratio ?? 0))}
      style:visibility={!display || display.ratio === 0 ? 'hidden' : 'visible'}
    />
  </svg>
  <div class="status" aria-hidden="true">
    {#if error}{m.mini_sync_error()}
    {:else if engaged}{hintText}
    {:else if !timerSnapshot || !timerSnapshot.is_running}
      <svg width="10" height="10" viewBox="0 0 12 12" fill="currentColor">
        {#if timerSnapshot?.is_paused}<rect x="3" y="2" width="2" height="8" /><rect
            x="7"
            y="2"
            width="2"
            height="8"
          />{:else}<path d="M3 1 10 6 3 11Z" />{/if}
      </svg>
    {/if}
  </div>
  <button
    class="time"
    style:font-size="{display?.fontSize ?? 24}px"
    disabled={busy || !timerSnapshot || error}
    aria-label={error ? m.mini_sync_error() : toggleLabel}
    onclick={onToggle}
    onkeydown={(event) => {
      if (event.repeat) event.preventDefault();
    }}
    onpointerenter={() => (pointerHint = 'toggle')}
    onpointerleave={() => (pointerHint = null)}
    onfocus={() => (focusHint = 'toggle')}
    onblur={() => (focusHint = null)}>{display?.text ?? '--:--'}</button
  >
  <div class="actions">
    <button
      class="skip"
      disabled={busy || !timerSnapshot || error}
      aria-label={m.tooltip_skip()}
      onclick={onSkip}
      onkeydown={(event) => {
        if (event.repeat) event.preventDefault();
      }}
      onpointerenter={() => (pointerHint = 'skip')}
      onpointerleave={() => (pointerHint = null)}
      onfocus={() => (focusHint = 'skip')}
      onblur={() => (focusHint = null)}
    >
      <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true"
        ><polygon points="1,1 10,8 1,15" /><rect x="12" y="1" width="3" height="14" /></svg
      >
    </button>
    <button
      class="restore"
      aria-label={m.mini_restore()}
      onclick={onRestore}
      onkeydown={(event) => {
        if (event.repeat) event.preventDefault();
      }}
      onpointerenter={() => (pointerHint = 'restore')}
      onpointerleave={() => (pointerHint = null)}
      onfocus={() => (focusHint = 'restore')}
      onblur={() => (focusHint = null)}
    >
      <svg width="12" height="12" viewBox="0 0 14 14" fill="none" aria-hidden="true"
        ><path
          d="M8 2h4v4M12 2 7 7M6 12H2V8M2 12l5-5"
          stroke="currentColor"
          stroke-width="1.5"
          stroke-linecap="round"
          stroke-linejoin="round"
        /></svg
      >
    </button>
  </div>
</div>

<style>
  .mini {
    position: relative;
    width: 112px;
    height: 112px;
    background: var(--color-background);
    box-shadow: inset 0 0 0 1px var(--color-separator);
    overflow: hidden;
    touch-action: none;
    cursor: grab;
  }
  .mini.locked {
    cursor: default;
  }
  .ring {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }
  .progress {
    transition: none;
  }
  .progress.smooth {
    transition: stroke-dashoffset 150ms linear;
  }
  .progress.paused {
    opacity: 0.55;
  }
  .status {
    position: absolute;
    left: 20px;
    top: 19px;
    width: 72px;
    height: 14px;
    display: flex;
    justify-content: center;
    align-items: center;
    font-size: 10px;
    line-height: 1;
    color: var(--color-foreground-darker);
    white-space: nowrap;
    pointer-events: none;
  }
  button {
    border: 0;
    background: transparent;
    color: var(--color-foreground-darker);
    cursor: pointer;
    border-radius: 4px;
    padding: 0;
  }
  .time {
    position: absolute;
    left: 14px;
    top: 34px;
    width: 84px;
    height: 30px;
    display: grid;
    place-items: center;
    font-family: 'Mona Sans Mono', monospace;
    font-weight: 300;
    font-stretch: 85%;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
    color: var(--color-foreground);
    line-height: 1;
  }
  .time:disabled {
    cursor: default;
  }
  .actions {
    opacity: 0;
    pointer-events: none;
    transition: opacity 120ms;
  }
  .engaged .actions,
  .mini:focus-within .actions {
    opacity: 1;
    pointer-events: auto;
  }
  .actions button {
    position: absolute;
    top: 66px;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    transition:
      background 120ms,
      color 120ms;
  }
  .skip {
    left: 28px;
  }
  .restore {
    left: 60px;
  }
  .actions button:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  .actions button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 1px;
  }
  @media (prefers-reduced-motion: reduce) {
    .progress.smooth,
    .actions,
    .actions button {
      transition: none;
    }
  }
</style>
