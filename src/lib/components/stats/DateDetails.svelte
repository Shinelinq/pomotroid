<script lang="ts">
  import * as m from '$paraglide/messages.js';
  let {
    date,
    lines,
    onclear,
    hint,
    clearLabel,
    onrecords,
    entryKey,
    entryLabel,
  }: {
    date: string | null;
    lines: string[];
    onclear: () => void;
    hint?: string;
    clearLabel?: string;
    onrecords?: () => void;
    entryKey?: string;
    entryLabel?: string;
  } = $props();
</script>

<div class="details" class:with-entry={!!onrecords} aria-live="polite" aria-atomic="true">
  {#if date}
    <div class="detail-copy">
      <div class="date-line">
        <span>{date}</span><button
          class="clear-selection"
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
    </div>
    {#if onrecords}<button class="record-entry" data-detail-entry={entryKey} onclick={onrecords}
        >{entryLabel ?? m.detail_open_day()}</button
      >{/if}
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
  .details.with-entry {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .detail-copy {
    min-width: 0;
    flex: 1;
  }
  .record-entry {
    position: static;
    transform: none;
    width: auto;
    min-height: 28px;
    height: auto;
    padding: 4px 6px;
    font: inherit;
    font-size: 12px;
    white-space: nowrap;
  }
  @media (max-width: 520px) {
    .details.with-entry {
      flex-wrap: wrap;
    }
    .with-entry .detail-copy {
      flex-basis: 100%;
    }
    .record-entry {
      margin-left: auto;
    }
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
  button.clear-selection {
    position: absolute;
    right: 0;
    top: 50%;
    transform: translateY(-50%);
    width: 24px;
    height: 24px;
  }
  button {
    display: grid;
    place-items: center;
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
      transition: none;
    }
  }
</style>
