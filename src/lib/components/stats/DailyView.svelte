<script lang="ts">
  import { untrack } from 'svelte';
  import type { ChartSelection, DetailEntry } from './sessionDetails';
  import type { DailyStats } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import StatsSummary from './StatsSummary.svelte';
  import StatsHelp from './StatsHelp.svelte';
  import StatsTooltip from './StatsTooltip.svelte';
  import DateDetails from './DateDetails.svelte';
  import { createChartInteraction } from './interaction.svelte';
  import { measurePlot } from './measurePlot';
  import { chartScale, formatDuration, localDate, hourRange } from './stats';

  let {
    today,
    date,
    initialSelection,
    onopen,
  }: {
    today: DailyStats;
    date: string;
    initialSelection?: ChartSelection | null;
    onopen?: (entry: DetailEntry) => void;
  } = $props();
  const interaction = createChartInteraction(untrack(() => initialSelection?.pinned ?? null));
  const tooltipId = 'daily-hour-tooltip';
  let chart = $state<SVGSVGElement>();
  let plot = $state({ width: 0, height: 190 });
  let focusedHour = $state(
    untrack(() =>
      initialSelection?.focused == null ? new Date().getHours() : Number(initialSelection.focused)
    )
  );
  const number = $derived(new Intl.NumberFormat(getLocale()));
  const fullDate = $derived(new Intl.DateTimeFormat(getLocale(), { dateStyle: 'full' }));
  const shortDate = $derived(
    new Intl.DateTimeFormat(getLocale(), { year: 'numeric', month: 'short', day: 'numeric' })
  );
  const hours = $derived(
    today.by_hour.map((rounds, hour) => ({ hour, rounds, seconds: today.by_hour_focus_secs[hour] }))
  );
  const scale = $derived(chartScale(Math.max(0, ...today.by_hour), 'rounds', plot.height - 42));
  const width = $derived(Math.max(1, plot.width));
  const height = $derived(Math.max(1, plot.height));
  const left = $derived(Math.max(36, ...scale.ticks.map((n) => number.format(n).length * 6.5 + 8)));
  const interval = $derived(Math.max(1, (width - left - 8) / 24));
  const barWidth = $derived(Math.min(24, interval * 0.55));
  const baseline = $derived(height - 30);
  const plotHeight = $derived(Math.max(1, baseline - 12));
  const labelStep = $derived(width >= 900 ? 3 : 6);
  const axisHours = $derived(Array.from({ length: 24 / labelStep + 1 }, (_, i) => i * labelStep));
  const pinned = $derived(interaction.pinned === null ? null : hours[Number(interaction.pinned)]);
  const preview = $derived(
    interaction.preview === null ? null : hours[Number(interaction.preview.date)]
  );
  const range = (hour: number) => m.stats_hour_range(hourRange(hour));
  const values = (hour: { rounds: number; seconds: number }) => [
    `${m.stats_rounds()}: ${number.format(hour.rounds)}`,
    m.stats_hour_focus({ duration: formatDuration(hour.seconds) }),
  ];
  const description = (hour: (typeof hours)[number]) => [
    fullDate.format(localDate(date)),
    range(hour.hour),
    ...values(hour),
    m.stats_start_hour_help(),
  ];
  let previousRange: string | undefined;
  $effect(() => {
    const range = date;
    if (previousRange !== undefined && previousRange !== range) {
      interaction.clear();
      focusedHour = new Date().getHours();
    }
    previousRange = range;
  });
  export function selection(): ChartSelection {
    return { pinned: interaction.pinned, focused: String(focusedHour) };
  }
  function show(event: PointerEvent | FocusEvent, hour: number, immediate = false) {
    interaction.show(String(hour), event.currentTarget as Element, immediate);
  }
  function navigate(event: KeyboardEvent, hour: number) {
    let next = hour;
    if (event.key === 'ArrowLeft') next = Math.max(0, hour - 1);
    else if (event.key === 'ArrowRight') next = Math.min(23, hour + 1);
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = 23;
    else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      interaction.toggle(String(hour));
      return;
    } else return;
    event.preventDefault();
    focusedHour = next;
    chart?.querySelector<SVGGElement>(`[data-hour="${next}"]`)?.focus();
  }
</script>

<svelte:window
  onkeydown={interaction.escape}
  onclick={(event) => {
    const target = event.target as Element;
    if (chart?.contains(target) && !target.closest('[data-hour]')) interaction.clear();
  }}
/>
<div class="view">
  <div class="summary-row">
    <StatsSummary
      items={[
        { label: m.stats_rounds(), value: number.format(today.rounds) },
        // Preserve the existing summary's minute rounding; hourly detail uses exact backend seconds.
        { label: m.stats_focus_time(), value: formatDuration(today.focus_mins * 60) },
        {
          label: m.stats_completion(),
          value:
            today.completion_rate === null ? '—' : `${Math.round(today.completion_rate * 100)}%`,
        },
      ]}
    />
  </div>
  <div class="chart-section">
    <div class="toolbar">
      <div class="chart-title">
        <h2>{m.stats_hourly_rounds()}</h2>
        <StatsHelp label={m.stats_hourly_rounds()} text={m.stats_start_hour_help()} />
      </div>
      <div class="day-navigation">
        <span>{shortDate.format(localDate(date))}</span>
        {#if onopen}<button
            class="record-entry"
            data-detail-entry="today-day"
            onclick={() => onopen?.({ date, hour: null, focusKey: 'today-day' })}
            >{m.detail_open_day()}</button
          >{/if}
      </div>
    </div>
    <div class="plot" use:measurePlot={(size) => (plot = size)}>
      <svg
        bind:this={chart}
        {width}
        {height}
        viewBox="0 0 {width} {height}"
        role="group"
        aria-label={m.stats_hour_chart_help()}
      >
        {#each [0, ...scale.ticks] as value}
          {@const y = baseline - (value / scale.maximum) * plotHeight}
          <line x1={left} x2={width - 8} y1={y} y2={y} class="gridline" />
          <text x={left - 8} {y} dy="3" text-anchor="end" class="axis-label"
            >{number.format(value)}</text
          >
        {/each}
        {#each hours as hour}
          {@const x = left + (hour.hour + 0.5) * interval}
          {@const barHeight = (hour.rounds / scale.maximum) * plotHeight}
          <g
            role="button"
            tabindex={hour.hour === focusedHour ? 0 : -1}
            data-hour={hour.hour}
            aria-label={description(hour).join(', ')}
            aria-pressed={interaction.pinned === String(hour.hour)}
            aria-describedby={preview?.hour === hour.hour ? tooltipId : undefined}
            onclick={() => {
              focusedHour = hour.hour;
              interaction.toggle(String(hour.hour));
            }}
            onpointerenter={(event) => show(event, hour.hour)}
            onpointerleave={interaction.leave}
            onfocus={(event) => {
              focusedHour = hour.hour;
              show(event, hour.hour, true);
            }}
            onblur={interaction.leave}
            onkeydown={(event) => navigate(event, hour.hour)}
          >
            <rect
              class="hit"
              data-tooltip-hit
              x={left + hour.hour * interval + 1}
              y="3"
              width={Math.max(1, interval - 2)}
              height={Math.max(0, height - 6)}
              rx="2"
            />
            {#if barHeight > 0}<rect
                class="bar"
                data-tooltip-anchor
                x={x - barWidth / 2}
                y={baseline - barHeight}
                width={barWidth}
                height={barHeight}
                rx="2"
              />{/if}
            {#if interaction.pinned === String(hour.hour)}<line
                x1={x - 5}
                x2={x + 5}
                y1={height - 3}
                y2={height - 3}
                class="selected-line"
              />{/if}
          </g>
        {/each}
        {#each axisHours as hour}<text
            x={left + hour * interval}
            y={baseline + 20}
            text-anchor={hour === 0 ? 'start' : hour === 24 ? 'end' : 'middle'}
            class="axis-label">{hourRange(hour).start}</text
          >{/each}
        {#if today.rounds === 0}<text
            x={left + (width - left) / 2}
            y={12 + plotHeight * 0.4}
            text-anchor="middle"
            class="empty">{m.stats_no_sessions_today()}</text
          >{/if}
      </svg>
    </div>
    <DateDetails
      date={pinned ? `${fullDate.format(localDate(date))} · ${range(pinned.hour)}` : null}
      lines={pinned ? values(pinned) : []}
      hint={m.stats_hour_detail_hint()}
      clearLabel={m.stats_clear_hour()}
      onclear={interaction.unpin}
      entryKey="today-hour"
      entryLabel={m.detail_open_hour()}
      onrecords={pinned && onopen
        ? () => {
            if (pinned) onopen?.({ date, hour: pinned.hour, focusKey: 'today-hour' });
          }
        : undefined}
    />
  </div>
  {#if interaction.preview && preview}<StatsTooltip
      id={tooltipId}
      anchor={interaction.preview.anchor}
      lines={description(preview)}
      onenter={interaction.keep}
      onleave={interaction.leave}
    />{/if}
</div>

<style>
  .view {
    flex: 1 0 auto;
    min-height: 100%;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .summary-row {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-separator);
  }
  .chart-section {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding: var(--stats-block-gap, 20px) var(--stats-pad, 24px) 0;
  }
  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
    color: var(--color-foreground-darker);
    font-size: 0.72rem;
  }
  .day-navigation {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    flex-wrap: wrap;
    gap: 4px 8px;
  }
  .record-entry {
    min-height: 28px;
    padding: 4px 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .record-entry:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }
  .record-entry:focus-visible {
    outline: 1px solid var(--color-foreground);
    outline-offset: 1px;
  }
  .chart-title {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  h2 {
    font-size: 0.75rem;
    font-weight: 600;
  }
  .plot {
    position: relative;
    flex: 1 0 190px;
    min-height: 190px;
    min-width: 0;
    margin: var(--stats-chart-gap, 16px) 0 8px;
  }
  svg {
    position: absolute;
    inset: 0;
    display: block;
    overflow: visible;
  }
  .gridline {
    stroke: var(--color-separator);
    stroke-width: 1;
  }
  .axis-label {
    fill: var(--color-foreground-darker);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }
  g {
    outline: none;
    cursor: pointer;
  }
  .hit {
    fill: transparent;
    stroke: transparent;
    stroke-width: 2;
  }
  .bar {
    fill: var(--color-focus-round);
    opacity: 0.85;
    pointer-events: none;
    transition: opacity 120ms;
  }
  g:hover .bar {
    opacity: 1;
  }
  g:focus-visible .hit {
    stroke: color-mix(in oklch, var(--color-foreground) 45%, transparent);
  }
  .selected-line {
    stroke: var(--color-focus-round);
    stroke-width: 2;
  }
  .empty {
    fill: var(--color-foreground-darker);
    font-size: 0.75rem;
    pointer-events: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
</style>
