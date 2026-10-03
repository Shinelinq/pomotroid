<script lang="ts">
  import * as m from '$paraglide/messages.js';
  import type { Metric } from './stats';
  let { value = $bindable<Metric>('time') }: { value?: Metric } = $props();
</script>

<div class="metric-switch" role="group" aria-label={m.stats_metric()}>
  <button
    class:active={value === 'time'}
    aria-pressed={value === 'time'}
    onclick={() => (value = 'time')}>{m.stats_focus_time()}</button
  >
  <button
    class:active={value === 'rounds'}
    aria-pressed={value === 'rounds'}
    onclick={() => (value = 'rounds')}>{m.stats_metric_rounds()}</button
  >
</div>

<style>
  .metric-switch {
    display: flex;
    gap: 2px;
    flex-wrap: wrap;
  }
  button {
    font: inherit;
    font-size: 0.72rem;
    font-weight: 600;
    min-height: 28px;
    padding: 4px 10px;
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: 4px 4px 0 0;
    background: transparent;
    color: var(--color-foreground-darker);
    cursor: pointer;
    transition:
      color 120ms,
      background 120ms,
      border-color 120ms;
  }
  button:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  button.active {
    color: var(--color-focus-round);
    border-bottom-color: var(--color-focus-round);
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    button {
      transition: none;
    }
  }
</style>
