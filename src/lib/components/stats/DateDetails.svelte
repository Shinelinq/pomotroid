<script lang="ts">
  import * as m from '$paraglide/messages.js';
  let {
    date,
    lines,
    onclear,
    hint,
    clearLabel,
  }: {
    date: string | null;
    lines: string[];
    onclear: () => void;
    hint?: string;
    clearLabel?: string;
  } = $props();
</script>

<div class="details" aria-live="polite" aria-atomic="true">
  {#if date}
    <div class="date-line">
      <span>{date}</span><button
        onclick={onclear}
        aria-label={clearLabel ?? m.stats_clear_selection()}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"
          ><path
            d="m2 2 8 8M10 2l-8 8"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          /></svg
        >
      </button>
    </div>
    <div class="values">
      {#each lines as line, i}{#if i > 0}<span class="separator" aria-hidden="true">·</span
          >{/if}<span>{line}</span>{/each}
    </div>
  {:else}<p>{hint ?? m.stats_detail_hint()}</p>{/if}
</div>

<style>
  .details {
    border-top: 1px solid var(--color-separator);
    position: relative;
    min-height: var(--stats-detail-min, 48px);
    padding: var(--stats-detail-y, 6px) 32px var(--stats-detail-y, 6px) 0;
    font-size: 0.75rem;
    line-height: 1.3;
  }
  .date-line {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    font-weight: 600;
  }
  .values {
    display: flex;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 8px;
    color: var(--color-foreground-darker);
    font-variant-numeric: tabular-nums;
  }
  .values > span:not(.separator) {
    white-space: nowrap;
  }
  p {
    color: var(--color-foreground-darker);
    padding-top: 4px;
  }
  button {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    flex-shrink: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    cursor: pointer;
    transition:
      background 120ms,
      color 120ms;
  }
  button:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    button {
      position: absolute;
      right: 0;
      top: 50%;
      transform: translateY(-50%);
      transition: none;
    }
  }
</style>
