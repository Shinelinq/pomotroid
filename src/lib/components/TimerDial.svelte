<script lang="ts">
  // SVG arc dial showing timer progress.
  // Replicates the original Pomotroid dial: fills from 0% to 100% as time elapses.
  // Uses Svelte tweened store for smooth animation.
  import { tweened } from 'svelte/motion';
  import { cubicOut } from 'svelte/easing';
  import type { TimerState } from '$lib/types';

  interface Props {
    snap: TimerState;
    countdown?: boolean;
    mainWindow?: boolean;
  }

  let { snap, countdown = false, mainWindow = false }: Props = $props();

  // SVG constants (matching original Pomotroid geometry)
  const CIRCUMFERENCE = 691.15; // 2π × 110 ≈ 691.15

  // Tweened offset: starts at full circumference (invisible), animates toward 0 (full arc).
  const dashOffset = tweened(CIRCUMFERENCE, { duration: 800, easing: cubicOut });

  // Round-type → CSS custom property for stroke color.
  function strokeColor(rt: string): string {
    if (rt === 'work') return 'var(--color-focus-round)';
    if (rt === 'short-break') return 'var(--color-short-round)';
    return 'var(--color-long-round)';
  }

  // Track previous round to detect round changes and snap the animation.
  // Not reactive — only used for comparison inside $effect.
  let prevRound = $state<string>('');

  $effect(() => {
    const rt = snap.round_type;
    const progress = snap.total_secs > 0 ? snap.elapsed_secs / snap.total_secs : 0;

    // Elapsed mode: arc grows from empty → full (offset counts down to 0).
    // Countdown mode: arc shrinks from full → empty (offset counts up to CIRCUMFERENCE).
    const target = countdown ? CIRCUMFERENCE * progress : CIRCUMFERENCE * (1 - progress);
    const startOffset = countdown ? 0 : CIRCUMFERENCE;

    // On round change: snap to start position immediately.
    if (rt !== prevRound) {
      dashOffset.set(startOffset, { duration: 0 });
      prevRound = rt;
    } else {
      dashOffset.set(target);
    }
  });
</script>

<!-- The existing path has a 220-unit diameter. A 9.209-unit stroke in a
     229.209-unit viewBox produces a 224px outer diameter and 9px stroke at s=1.
     Only the viewport scales; the path, dash length and stroke stay in SVG units. -->
<svg
  class="dial"
  class:main-window={mainWindow}
  viewBox={mainWindow ? '0.395349 0.395349 229.209302 229.209302' : '0 0 230 230'}
  aria-hidden="true"
>
  <!-- Background track -->
  <path
    class="track"
    d="M115,5c60.8,0,110,49.2,110,110s-49.2,110-110,110S5,175.8,5,115S54.2,5,115,5"
    fill="none"
    stroke="var(--color-background-light)"
    stroke-width="2"
  />
  <!-- Progress arc -->
  <path
    class="progress"
    d="M115,5c60.8,0,110,49.2,110,110s-49.2,110-110,110S5,175.8,5,115S54.2,5,115,5"
    fill="none"
    stroke={strokeColor(snap.round_type)}
    stroke-width={mainWindow ? 9.209302 : 10}
    stroke-linecap="round"
    stroke-dasharray={CIRCUMFERENCE}
    stroke-dashoffset={$dashOffset}
  />
</svg>

<style>
  .dial {
    width: var(--dial-size, 220px);
    height: var(--dial-size, 220px);
    display: block;
  }
  .dial.main-window {
    width: var(--main-timer-dial);
    height: var(--main-timer-dial);
    flex: none;
  }
</style>
