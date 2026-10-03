<script lang="ts">
  import * as m from '$paraglide/messages.js';
  import { statsDistribution } from '$lib/ipc';
  import type { Distribution, DistributionRow } from '$lib/data/types';
  import { percentage } from '$lib/data/format';
  import { formatDuration } from './stats';
  import { exactDuration } from './sessionDetails';
  let {
    start,
    end,
    label,
    visible,
    returnName = null,
    expanded = $bindable(false),
    all = $bindable(false),
    revision,
    cache,
    onselect,
    onback,
  }: {
    start: string;
    end: string;
    label: string;
    visible: boolean;
    returnName?: string | null;
    expanded?: boolean;
    all?: boolean;
    revision: number;
    cache: Map<string, Distribution>;
    onselect: (row: DistributionRow) => void;
    onback: () => void;
  } = $props();
  let result = $state<Distribution | null>(null);
  let loading = $state(false);
  let failed = $state(false);
  let retry = $state(0);
  $effect(() => {
    const key = `${start}/${end}/${revision}`;
    retry;
    if (!visible || !expanded) return;
    const cached = cache.get(key) ?? null;
    let alive = true;
    failed = false;
    result = cached;
    loading = !cached;
    if (!cached)
      void statsDistribution(start, end)
        .then((value) => {
          if (alive) {
            cache.set(key, value);
            result = value;
          }
        })
        .catch(() => {
          if (alive) failed = true;
        })
        .finally(() => {
          if (alive) loading = false;
        });
    return () => {
      alive = false;
    };
  });
  const rows = $derived(result ? (all ? result.rows : result.rows.slice(0, 5)) : []);
  const remainder = $derived(
    result?.rows.slice(5).reduce((sum, row) => sum + row.focus_secs, 0) ?? 0
  );
</script>

{#if visible || returnName}
  <section class="distribution">
    {#if returnName}
      <button class="heading" data-distribution-return onclick={onback}
        >{m.distribution_back({ name: returnName })}</button
      >
    {:else}
      <div class="header">
        <button
          class="heading"
          data-distribution-toggle
          aria-expanded={expanded}
          onclick={() => (expanded = !expanded)}
          >{expanded ? m.distribution_title() : m.distribution_open()}
          <span aria-hidden="true">{expanded ? '▾' : '▸'}</span></button
        >{#if expanded}<span class="range">{label}</span>{/if}
      </div>
      {#if expanded}
        <p class="hint">
          {m.distribution_hint()}{result && result.total_focus_secs
            ? ` · ${formatDuration(result.total_focus_secs)}`
            : ''}
        </p>
        {#if loading}<p class="hint" role="status">{m.stats_loading()}</p>{:else if failed}<p
            class="hint"
            role="status"
          >
            {m.stats_load_error()} <button onclick={() => retry++}>{m.stats_retry()}</button>
          </p>
        {:else if result}
          {#if result.total_focus_secs === 0}<p class="hint">{m.distribution_empty()}</p>{:else}
            <div class="rows">
              {#each rows as row (row.category_id)}
                <button
                  class="row"
                  title={row.name ?? m.category_uncategorized()}
                  onclick={() => onselect(row)}
                >
                  <span class="name"
                    >{row.name ?? m.category_uncategorized()}{#if row.archived}<small>
                        · {m.category_archived()}</small
                      >{/if}</span
                  >
                  <span class="track" aria-hidden="true"
                    ><span style:width={`${(row.focus_secs / result.total_focus_secs) * 100}%`}
                    ></span></span
                  >
                  <span class="duration">{exactDuration(row.focus_secs)}</span><span class="percent"
                    >{percentage(row.focus_secs, result.total_focus_secs)}</span
                  >
                </button>
              {/each}
            </div>
            {#if result.rows.length > 5}<button class="more" onclick={() => (all = !all)}
                >{all
                  ? m.distribution_less()
                  : m.distribution_more({
                      count: result.rows.length - 5,
                      percent: percentage(remainder, result.total_focus_secs),
                    })}</button
              >{/if}
            <p class="hint rounding">{m.distribution_rounding()}</p>
          {/if}
        {/if}
      {/if}
    {/if}
  </section>
{/if}

<style>
  .distribution {
    margin: 0 var(--stats-pad, 24px) 12px;
    flex: none;
    min-width: 0;
    font-size: 12px;
  }
  button {
    font: inherit;
    cursor: pointer;
    border: 0;
    border-radius: 4px;
    color: var(--color-foreground);
    background: transparent;
    min-height: 28px;
    padding: 4px;
  }
  button:hover {
    background: var(--color-hover);
  }
  button:focus-visible {
    outline: 1px solid var(--color-focus-round);
    outline-offset: 1px;
  }
  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .heading {
    text-align: left;
    overflow-wrap: anywhere;
  }
  .range,
  .hint,
  small {
    font-size: 11px;
    color: var(--color-foreground-darker);
  }
  .hint {
    margin: 4px 0 8px;
    line-height: 1.5;
  }
  .rows {
    display: flex;
    flex-direction: column;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(80px, 130px) minmax(24px, 1fr) minmax(88px, max-content) 48px;
    align-items: center;
    gap: 12px;
    min-height: 38px;
    width: 100%;
    text-align: left;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .track {
    height: 5px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--color-separator) 55%, transparent);
  }
  .track > span {
    display: block;
    height: 5px;
    border-radius: 2px;
    background: var(--color-focus-round);
  }
  .duration,
  .percent {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .percent {
    color: var(--color-foreground-darker);
    font-size: 11px;
  }
  .more {
    font-size: 11px;
    color: var(--color-foreground-darker);
  }
  .rounding {
    margin-top: 6px;
  }
  @media (max-width: 620px) {
    .row {
      grid-template-columns: minmax(0, 1fr) auto 48px;
      gap: 4px 8px;
      padding: 6px 4px;
    }
    .track {
      grid-row: 2;
      grid-column: 1/-1;
    }
    .duration {
      grid-column: 2;
      grid-row: 1;
    }
    .percent {
      grid-column: 3;
      grid-row: 1;
    }
  }
</style>
