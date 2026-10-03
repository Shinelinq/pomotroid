<script lang="ts">
  import StatsHelp from './StatsHelp.svelte';
  let {
    items,
    compact = false,
  }: { items: { label: string; value: string; hint?: string }[]; compact?: boolean } = $props();
  const wide = $derived(items.some((item) => item.value.length > 14));
</script>

<div class="summary-container">
  <dl class="summary" class:compact class:wide style="--columns: {items.length}">
    {#each items as item}
      <div class="item">
        <dt>
          <span>{item.label}</span>{#if item.hint}<StatsHelp
              label={item.label}
              text={item.hint}
            />{/if}
        </dt>
        <dd class:long={item.value.length > 9}>{item.value}</dd>
      </div>
    {/each}
  </dl>
</div>

<style>
  .summary-container {
    container-type: inline-size;
  }
  .summary {
    display: grid;
    grid-template-columns: repeat(var(--columns), minmax(0, 1fr));
    padding: var(--stats-summary-y, 22px) var(--stats-pad, 24px);
    min-height: var(--stats-summary-min, 108px);
    row-gap: 16px;
  }
  .summary.compact {
    padding-top: var(--stats-summary-y, 22px);
    padding-bottom: var(--stats-summary-y, 22px);
  }
  .item {
    position: relative;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--stats-summary-gap, 6px);
    justify-content: center;
    padding: 0 6px;
    text-align: center;
  }
  .item + .item::before {
    content: '';
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--color-separator);
  }
  dt {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 3px;
    line-height: 1.3;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--color-foreground-darker);
    overflow-wrap: anywhere;
  }
  dd {
    font-size: var(--stats-number-size, 2rem);
    font-weight: 700;
    line-height: 1.1;
    font-variant-numeric: tabular-nums;
    letter-spacing: -0.02em;
    white-space: nowrap;
    color: var(--color-foreground);
  }
  dd.long {
    font-size: 1.5rem;
  }
  .wide {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .wide .item:nth-child(3)::before {
    display: none;
  }
  @container (max-width: 36rem) {
    .summary {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .item:nth-child(3)::before {
      display: none;
    }
  }
</style>
