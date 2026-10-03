<script lang="ts">
  // Displays the remaining time (MM:SS).
  import type { TimerState } from '$lib/types';

  interface Props {
    state: TimerState;
    mainWindow?: boolean;
  }

  let { state, mainWindow = false }: Props = $props();

  let remaining = $derived(state.total_secs - state.elapsed_secs);
  let minutes = $derived(Math.floor(remaining / 60));
  let seconds = $derived(remaining % 60);
  let display = $derived(`${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`);
  // Tabular mono glyphs fit six characters at the standard size. Only longer
  // values shrink, based on character count (never on the changing digit widths).
  const timeFit = $derived(Math.min(1, 6 / display.length));
</script>

<div class="display" class:main-window={mainWindow} style:--time-fit={timeFit}>
  <span class="time">{display}</span>
</div>

<style>
  .display {
    /* Fill the dial-stack and flex-center the time. */
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
  }

  .time {
    font-family: 'Mona Sans Mono', monospace;
    font-size: 2.8rem;
    font-weight: 300;
    font-stretch: 85%;
    letter-spacing: -0.02em;
    color: var(--color-foreground);
  }

  .main-window .time {
    font-size: calc(var(--main-timer-time) * var(--time-fit));
    line-height: 1;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
</style>
