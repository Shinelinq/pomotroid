<script lang="ts">
  import { untrack } from 'svelte';
  import type { Snippet } from 'svelte';
  import type { ChartSelection, DetailEntry } from './sessionDetails';
  import type { HeatmapStats, HeatmapEntry } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import MetricSwitch from './MetricSwitch.svelte';
  import StatsSummary from './StatsSummary.svelte';
  import StatsTooltip from './StatsTooltip.svelte';
  import DateDetails from './DateDetails.svelte';
  import { createChartInteraction } from './interaction.svelte';
  import { measureWidth } from './measureWidth';
  import {
    buildYear,
    summarizeYear,
    formatDuration,
    localDate,
    heatLevel,
    moveDate,
    type Metric,
    type CalendarCell,
  } from './stats';

  let {
    heatmap,
    today,
    loading = false,
    initialSelection,
    onopen,
    scopeLabel = null,
    metric = $bindable<Metric>('time'),
    selectedYear = $bindable(new Date().getFullYear()),
    distribution,
  }: {
    heatmap: HeatmapStats | null;
    today: string;
    loading?: boolean;
    initialSelection?: ChartSelection | null;
    onopen?: (entry: DetailEntry) => void;
    scopeLabel?: string | null;
    metric?: Metric;
    selectedYear?: number;
    distribution?: Snippet;
  } = $props();
  const interaction = createChartInteraction(untrack(() => initialSelection?.pinned ?? null));
  const legendInteraction = createChartInteraction();
  const tooltipId = 'yearly-date-tooltip';
  let chart = $state<SVGSVGElement>();
  let containerWidth = $state(0);
  let focusedDate = $state<string | null>(untrack(() => initialSelection?.focused ?? null));
  const currentYear = $derived(Number(today.slice(0, 4)));
  const firstYear = $derived(
    Math.min(currentYear, ...(heatmap?.entries ?? []).map((d) => Number(d.date.slice(0, 4))))
  );
  const calendar = $derived(
    heatmap === null ? null : buildYear(heatmap.entries, selectedYear, today)
  );
  const annual = $derived(heatmap === null ? null : summarizeYear(heatmap.entries, selectedYear));
  const activeDays = $derived(
    heatmap === null ? null : heatmap.entries.filter((d) => d.count > 0).length
  );
  const validDates = $derived(
    new Set((calendar?.cells ?? []).filter((d) => d.valid).map((d) => d.date))
  );
  const tabDate = $derived(
    focusedDate && validDates.has(focusedDate)
      ? focusedDate
      : validDates.has(today)
        ? today
        : [...validDates][0]
  );
  const fullDate = $derived(new Intl.DateTimeFormat(getLocale(), { dateStyle: 'full' }));
  const shortDate = $derived(
    new Intl.DateTimeFormat(getLocale(), { month: 'short', day: 'numeric' })
  );
  const monthName = $derived(new Intl.DateTimeFormat(getLocale(), { month: 'short' }));
  const weekday = $derived(new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }));
  const number = $derived(new Intl.NumberFormat(getLocale()));
  const LEFT = 32;
  const TOP = 20;
  const cellSize = $derived(
    Math.max(
      6,
      Math.min(
        14,
        (containerWidth - LEFT - 8 - ((calendar?.weeks ?? 53) - 1) * 2) / (calendar?.weeks ?? 53)
      )
    )
  );
  const stride = $derived(cellSize + 2);
  const svgWidth = $derived(LEFT + (calendar?.weeks ?? 53) * stride - 2 + 8);
  const svgHeight = $derived(TOP + 7 * stride + 2);
  const fills = ['var(--heat-0)', 'var(--heat-1)', 'var(--heat-2)', 'var(--heat-3)'];
  const pinned = $derived(
    calendar?.cells.find((cell) => cell.date === interaction.pinned && cell.valid) ?? null
  );
  const preview = $derived(
    calendar?.cells.find((cell) => cell.date === interaction.preview?.date && cell.valid) ?? null
  );
  const details = (day: HeatmapEntry) => [
    `${m.stats_focus_time()}: ${formatDuration(day.focus_secs)}`,
    `${m.stats_rounds()}: ${number.format(day.count)}`,
  ];
  const legend = $derived(
    metric === 'time'
      ? [
          m.stats_heat_time_zero(),
          m.stats_heat_time_low(),
          m.stats_heat_time_mid(),
          m.stats_heat_time_high(),
        ]
      : [
          m.stats_heat_count_zero(),
          m.stats_heat_count_low(),
          m.stats_heat_count_mid(),
          m.stats_heat_count_high(),
        ]
  );

  let previousRange: number | undefined;
  $effect(() => {
    const range = selectedYear;
    if (previousRange !== undefined && previousRange !== range) {
      interaction.clear();
      legendInteraction.clear();
      focusedDate = null;
    }
    previousRange = range;
  });
  $effect(() => {
    if (heatmap && selectedYear > currentYear) selectedYear = currentYear;
  });
  export function selection(): ChartSelection {
    return { pinned: interaction.pinned, focused: focusedDate };
  }
  function show(event: PointerEvent | FocusEvent, cell: CalendarCell, immediate = false) {
    legendInteraction.hidePreview();
    interaction.show(cell.date, event.currentTarget as SVGRectElement, immediate);
  }
  function navigate(event: KeyboardEvent, date: string) {
    const offsets: Record<string, number> = {
      ArrowLeft: -7,
      ArrowRight: 7,
      ArrowUp: -1,
      ArrowDown: 1,
    };
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      interaction.toggle(date);
      return;
    }
    let next: string;
    if (event.key === 'Home') next = [...validDates][0];
    else if (event.key === 'End') next = [...validDates].at(-1)!;
    else if (event.key in offsets) next = moveDate(date, offsets[event.key], validDates);
    else return;
    event.preventDefault();
    focusedDate = next;
    chart?.querySelector<SVGRectElement>(`[data-day="${next}"]`)?.focus();
  }
</script>

<svelte:window
  onkeydown={(event) => {
    interaction.escape(event);
    legendInteraction.escape(event);
  }}
  onclick={(event) => {
    const target = event.target as Element;
    if (chart?.contains(target) && !target.closest('[data-day]')) interaction.clear();
  }}
/>
<div class="view">
  <div class="year-section">
    <div class="year-group">
      <div class="toolbar">
        <div class="year-nav" role="group" aria-label={m.stats_year_navigation()}>
          <button
            disabled={heatmap === null || selectedYear <= firstYear}
            onclick={() => selectedYear--}
            aria-label={m.stats_prev_year()}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"
              ><path
                d="m9 2-5 5 5 5"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
          <span class="year-label" aria-live="polite">{selectedYear}</span>
          <button
            disabled={heatmap === null || selectedYear >= currentYear}
            onclick={() => selectedYear++}
            aria-label={m.stats_next_year()}
          >
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"
              ><path
                d="m5 2 5 5-5 5"
                stroke="currentColor"
                stroke-width="1.5"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
        </div>
        <MetricSwitch bind:value={metric} />
      </div>
      <div class="heatmap-wrap" use:measureWidth={(width) => (containerWidth = width)}>
        {#if calendar}
          <svg
            bind:this={chart}
            width={svgWidth}
            height={svgHeight}
            viewBox="0 0 {svgWidth} {svgHeight}"
            class="heatmap"
            role="group"
            aria-label={m.stats_year_chart_help({ year: String(selectedYear) })}
          >
            {#each calendar.months as month}<text
                x={LEFT + month.week * stride}
                y="10"
                class="month-label">{monthName.format(new Date(selectedYear, month.month, 1))}</text
              >{/each}
            {#each [1, 3, 5] as day}<text
                x={LEFT - 6}
                y={TOP + day * stride + cellSize / 2 + 3.5}
                text-anchor="end"
                class="weekday-label">{weekday.format(new Date(2000, 0, 2 + day))}</text
              >{/each}
            {#each calendar.cells as cell (cell.date)}
              {@const value = metric === 'time' ? cell.focus_secs : cell.count}
              {#if cell.valid}
                <rect
                  x={LEFT + cell.week * stride}
                  y={TOP + cell.weekday * stride}
                  width={cellSize}
                  height={cellSize}
                  rx="2"
                  style:fill={fills[heatLevel(value, metric)]}
                  class="cell"
                  class:selected={interaction.pinned === cell.date}
                  data-day={cell.date}
                  role="button"
                  tabindex={cell.date === tabDate ? 0 : -1}
                  aria-label={[fullDate.format(localDate(cell.date)), ...details(cell)].join(', ')}
                  aria-pressed={interaction.pinned === cell.date}
                  aria-describedby={preview?.date === cell.date ? tooltipId : undefined}
                  onpointerenter={(event) => show(event, cell)}
                  onpointerleave={interaction.leave}
                  onfocus={(event) => {
                    focusedDate = cell.date;
                    show(event, cell, true);
                  }}
                  onblur={interaction.leave}
                  onclick={() => {
                    focusedDate = cell.date;
                    interaction.toggle(cell.date);
                  }}
                  onkeydown={(event) => navigate(event, cell.date)}
                />
              {:else}
                <rect
                  x={LEFT + cell.week * stride}
                  y={TOP + cell.weekday * stride}
                  width={cellSize}
                  height={cellSize}
                  rx="2"
                  class="dimmed"
                  aria-hidden="true"
                />
              {/if}
            {/each}
          </svg>
        {:else}<div class="loading">{loading ? m.stats_loading() : '—'}</div>{/if}
      </div>
      <div class="legend" aria-label={m.stats_heat_legend()}>
        <span>{m.stats_legend_less()}</span>
        {#each legend as label, level}<button
            class="legend-cell"
            style:background={fills[level]}
            aria-label={label}
            aria-describedby={legendInteraction.preview?.date === String(level)
              ? 'yearly-legend-tooltip'
              : undefined}
            onpointerenter={(event) => {
              interaction.hidePreview();
              legendInteraction.show(String(level), event.currentTarget);
            }}
            onpointerleave={legendInteraction.leave}
            onfocus={(event) => {
              interaction.hidePreview();
              legendInteraction.show(String(level), event.currentTarget, true);
            }}
            onblur={legendInteraction.leave}
          ></button>{/each}
        <span>{m.stats_legend_more()}</span>
      </div>
      <dl class="annual">
        <div>
          <dt>{m.stats_year_focus()}</dt>
          <dd>{annual ? formatDuration(annual.seconds) : '—'}</dd>
        </div>
        <div>
          <dt>{m.stats_year_rounds()}</dt>
          <dd>{annual ? number.format(annual.rounds) : '—'}</dd>
        </div>
        <div>
          <dt>{m.stats_year_active()}</dt>
          <dd>{annual ? number.format(annual.active) : '—'}</dd>
        </div>
        <div>
          <dt>{m.stats_best_day()}</dt>
          <dd>{annual?.best ? formatDuration(annual.best.focus_secs) : '—'}</dd>
          <span class="best-date"
            >{annual?.best ? shortDate.format(localDate(annual.best.date)) : ''}</span
          >
        </div>
      </dl>
      <DateDetails
        date={pinned ? fullDate.format(localDate(pinned.date)) : null}
        lines={pinned ? details(pinned) : []}
        onclear={interaction.unpin}
        entryKey="year-day"
        onrecords={pinned && onopen
          ? () => {
              if (pinned) onopen?.({ date: pinned.date, focusKey: 'year-day' });
            }
          : undefined}
      />
    </div>
  </div>
  {@render distribution?.()}
  <div class="lifetime">
    <h2>
      {scopeLabel
        ? m.category_scope_title({ title: m.stats_lifetime(), name: scopeLabel })
        : m.stats_lifetime()}
    </h2>
    <StatsSummary
      compact
      items={[
        {
          label: m.stats_lifetime_rounds(),
          value: heatmap ? number.format(heatmap.total_rounds) : '—',
        },
        {
          label: m.stats_lifetime_focus(),
          value: heatmap ? formatDuration(heatmap.total_focus_secs) : '—',
        },
        {
          label: m.stats_active_days(),
          value: activeDays === null ? '—' : number.format(activeDays),
        },
        {
          label: m.stats_best_streak(),
          value: heatmap ? number.format(heatmap.longest_streak) : '—',
        },
      ]}
    />
  </div>
  {#if interaction.preview && preview}<StatsTooltip
      id={tooltipId}
      anchor={interaction.preview.anchor}
      lines={[fullDate.format(localDate(preview.date)), ...details(preview)]}
      onenter={interaction.keep}
      onleave={interaction.leave}
    />{/if}
  {#if legendInteraction.preview}<StatsTooltip
      id="yearly-legend-tooltip"
      anchor={legendInteraction.preview.anchor}
      lines={[legend[Number(legendInteraction.preview.date)]]}
      onenter={legendInteraction.keep}
      onleave={legendInteraction.leave}
    />{/if}
</div>

<style>
  .view {
    min-width: 0;
    min-height: 100%;
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
    --heat-0: color-mix(in oklch, var(--color-foreground) 6%, var(--color-background));
    --heat-1: color-mix(in oklch, var(--color-focus-round) 28%, var(--color-background));
    --heat-2: color-mix(in oklch, var(--color-focus-round) 60%, var(--color-background));
    --heat-3: var(--color-focus-round);
  }
  .year-section {
    padding: var(--stats-block-gap, 20px) var(--stats-pad, 24px) 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  .year-group {
    width: 100%;
  }
  .year-section > .year-group {
    flex-shrink: 0;
  }
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px 16px;
    flex-wrap: wrap;
  }
  .year-nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .year-label {
    font-size: 0.82rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .year-nav button {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    cursor: pointer;
    transition:
      color 120ms,
      background 120ms;
  }
  .year-nav button:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  .year-nav button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }
  .heatmap-wrap {
    margin-top: 12px;
    min-width: 0;
  }
  .heatmap {
    display: block;
    max-width: 100%;
    height: auto;
    overflow: visible;
  }
  .month-label,
  .weekday-label {
    font-size: 10px;
    fill: var(--color-foreground-darker);
  }
  .cell {
    cursor: pointer;
    outline: none;
    transition: opacity 120ms;
    stroke-width: 1.5;
  }
  .cell:hover {
    opacity: 0.75;
  }
  .cell.selected {
    stroke: var(--color-foreground);
    stroke-dasharray: 2 1;
  }
  .cell:focus-visible {
    stroke: var(--color-foreground);
    stroke-width: 2;
  }
  .dimmed {
    fill: var(--heat-0);
    opacity: 0.3;
  }
  .legend {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    min-height: 20px;
    margin-top: 8px;
    color: var(--color-foreground-darker);
    font-size: 0.72rem;
  }
  .legend-cell {
    width: 11px;
    height: 11px;
    border-radius: 2px;
    border: 0;
    cursor: help;
  }
  .annual {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
    margin: var(--stats-chart-gap, 16px) 0 8px;
  }
  .annual > div {
    min-width: 0;
  }
  dt {
    color: var(--color-foreground-darker);
    font-size: 0.68rem;
    font-weight: 600;
    margin-bottom: 6px;
  }
  dd {
    font-size: 0.95rem;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .best-date {
    font-size: 0.72rem;
    color: var(--color-foreground-darker);
  }
  .lifetime {
    flex-shrink: 0;
    border-top: 1px solid var(--color-separator);
  }
  h2 {
    padding: 8px var(--stats-pad, 24px) 0;
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--color-foreground-darker);
  }
  .loading {
    height: 120px;
    display: grid;
    place-items: center;
    color: var(--color-foreground-darker);
    font-size: 0.75rem;
  }
  @media (max-width: 640px) {
    .annual {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .cell,
    .year-nav button {
      transition: none;
    }
  }
</style>
