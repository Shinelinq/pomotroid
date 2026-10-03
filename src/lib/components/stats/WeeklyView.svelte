<script lang="ts">
  import { untrack } from 'svelte';
  import type { ChartSelection, DetailEntry } from './sessionDetails';
  import type { DayStat, StreakInfo } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import MetricSwitch from './MetricSwitch.svelte';
  import StatsSummary from './StatsSummary.svelte';
  import StatsTooltip from './StatsTooltip.svelte';
  import DateDetails from './DateDetails.svelte';
  import { createChartInteraction } from './interaction.svelte';
  import { measurePlot } from './measurePlot';
  import {
    buildWeek,
    summarizeWeek,
    formatDuration,
    formatRate,
    localDate,
    chartScale,
    moveDate,
    type Metric,
  } from './stats';

  let {
    week,
    streak,
    today,
    loading = false,
    initialSelection,
    onopen,
    metric = $bindable<Metric>('time'),
  }: {
    week: DayStat[] | null;
    streak: StreakInfo | null;
    today: string;
    loading?: boolean;
    initialSelection?: ChartSelection | null;
    onopen?: (entry: DetailEntry) => void;
    metric?: Metric;
  } = $props();
  const interaction = createChartInteraction(untrack(() => initialSelection?.pinned ?? null));
  const tooltipId = 'weekly-date-tooltip';
  let chart = $state<SVGSVGElement>();
  let plot = $state({ width: 0, height: 190 });
  let focusedDate = $state<string | null>(untrack(() => initialSelection?.focused ?? null));
  const days = $derived(week === null ? null : buildWeek(week, today));
  const summary = $derived(days === null ? null : summarizeWeek(days));
  const dates = $derived(new Set((days ?? []).map((d) => d.date)));
  const tabDate = $derived(focusedDate && dates.has(focusedDate) ? focusedDate : today);
  const weekday = $derived(new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }));
  const shortDate = $derived(
    new Intl.DateTimeFormat(getLocale(), { month: 'numeric', day: 'numeric' })
  );
  const fullDate = $derived(new Intl.DateTimeFormat(getLocale(), { dateStyle: 'full' }));
  const rangeDate = $derived(
    new Intl.DateTimeFormat(getLocale(), { year: 'numeric', month: 'short', day: 'numeric' })
  );
  const number = $derived(new Intl.NumberFormat(getLocale()));
  const valueOf = (day: DayStat) => (metric === 'time' ? day.focus_secs : day.rounds);
  const scale = $derived(
    chartScale(Math.max(0, ...(days ?? []).map(valueOf)), metric, plot.height - 50)
  );
  const tickLabel = (value: number) =>
    metric === 'time' ? formatDuration(value) : number.format(value);
  const left = $derived(
    Math.max(44, ...scale.ticks.map((value) => tickLabel(value).length * 6.5 + 8))
  );
  const width = $derived(Math.max(plot.width, 1));
  const svgHeight = $derived(Math.max(plot.height, 1));
  const interval = $derived(Math.max(1, (width - left - 4) / 7));
  const barWidth = $derived(Math.min(44, interval * 0.4));
  const BASELINE = $derived(svgHeight - 38);
  const PLOT_HEIGHT = $derived(Math.max(1, BASELINE - 12));
  const pinned = $derived(days?.find((day) => day.date === interaction.pinned) ?? null);
  const preview = $derived(days?.find((day) => day.date === interaction.preview?.date) ?? null);
  const details = (day: DayStat) => [
    `${m.stats_focus_time()}: ${formatDuration(day.focus_secs)}`,
    m.stats_completed_recorded({
      completed: number.format(day.rounds),
      recorded: number.format(day.started_rounds),
    }),
    `${m.stats_completion()}: ${formatRate(day.rounds, day.started_rounds)}`,
  ];

  let previousRange: string | undefined;
  $effect(() => {
    const range = today;
    if (previousRange !== undefined && previousRange !== range) {
      interaction.clear();
      focusedDate = null;
    }
    previousRange = range;
  });
  export function selection(): ChartSelection {
    return { pinned: interaction.pinned, focused: focusedDate };
  }
  function show(event: PointerEvent | FocusEvent, day: DayStat, immediate = false) {
    const element = event.currentTarget as SVGGElement;
    interaction.show(day.date, element, immediate);
  }
  function navigate(event: KeyboardEvent, date: string) {
    let next = date;
    if (event.key === 'ArrowLeft') next = moveDate(date, -1, dates);
    else if (event.key === 'ArrowRight') next = moveDate(date, 1, dates);
    else if (event.key === 'Home') next = days![0].date;
    else if (event.key === 'End') next = today;
    else if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      interaction.toggle(date);
      return;
    } else return;
    event.preventDefault();
    focusedDate = next;
    chart?.querySelector<SVGGElement>(`[data-day="${next}"]`)?.focus();
  }
</script>

<svelte:window
  onkeydown={interaction.escape}
  onclick={(event) => {
    const target = event.target as Element;
    if (chart?.contains(target) && !target.closest('[data-day]')) interaction.clear();
  }}
/>
<div class="view">
  <div class="summary-row">
    <StatsSummary
      items={[
        { label: m.stats_rounds(), value: summary ? number.format(summary.rounds) : '—' },
        { label: m.stats_focus_time(), value: summary ? formatDuration(summary.seconds) : '—' },
        {
          label: m.stats_completion(),
          value: summary ? formatRate(summary.rounds, summary.started) : '—',
        },
        {
          label: m.stats_active_average(),
          value: summary ? formatDuration(summary.average) : '—',
          hint: m.stats_active_average_hint(),
        },
      ]}
    />
  </div>
  <div class="chart-section">
    <div class="toolbar">
      <h2>{metric === 'time' ? m.stats_daily_focus() : m.stats_daily_rounds()}</h2>
      <MetricSwitch bind:value={metric} />
    </div>
    <div class="context">
      <span
        >{days
          ? m.stats_recent_range({
              start: rangeDate.format(localDate(days[0].date)),
              end: rangeDate.format(localDate(today)),
            })
          : '—'}</span
      >
      <span
        >{streak
          ? streak.current > 0
            ? m.stats_streak_days({ count: number.format(streak.current) })
            : m.stats_no_streak()
          : '—'}</span
      >
    </div>
    <div class="chart-wrap" use:measurePlot={(size) => (plot = size)}>
      {#if days}
        <svg
          bind:this={chart}
          width="100%"
          height={svgHeight}
          viewBox="0 0 {width} {svgHeight}"
          class="chart"
          role="group"
          aria-label={m.stats_week_chart_help()}
        >
          {#each [0, ...scale.ticks] as value}
            {@const y = BASELINE - (value / scale.maximum) * PLOT_HEIGHT}
            <line x1={left} y1={y} x2={width - 4} y2={y} class="gridline" />
            <text x={left - 8} {y} dy="3" text-anchor="end" class="axis-label"
              >{tickLabel(value)}</text
            >
          {/each}
          {#each days as day, i (day.date)}
            {@const x = left + interval * (i + 0.5)}
            {@const height = (valueOf(day) / scale.maximum) * PLOT_HEIGHT}
            <g
              role="button"
              tabindex={day.date === tabDate ? 0 : -1}
              data-day={day.date}
              aria-label={[fullDate.format(localDate(day.date)), ...details(day)].join(', ')}
              aria-pressed={interaction.pinned === day.date}
              aria-describedby={preview?.date === day.date ? tooltipId : undefined}
              class:today={day.date === today}
              class:selected={interaction.pinned === day.date}
              onclick={() => {
                focusedDate = day.date;
                interaction.toggle(day.date);
              }}
              onpointerenter={(event) => show(event, day)}
              onpointerleave={interaction.leave}
              onfocus={(event) => {
                focusedDate = day.date;
                show(event, day, true);
              }}
              onblur={interaction.leave}
              onkeydown={(event) => navigate(event, day.date)}
            >
              <rect
                class="hit"
                data-tooltip-hit
                x={left + i * interval + 3}
                y="3"
                width={interval - 6}
                height={Math.max(0, svgHeight - 6)}
                rx="2"
              />
              {#if height > 0}<path
                  class="bar"
                  data-tooltip-anchor
                  d="M{x - barWidth / 2} {BASELINE}V{BASELINE -
                    height +
                    Math.min(2, height)}q0 {-Math.min(2, height)} {Math.min(2, height)} {-Math.min(
                    2,
                    height
                  )}h{barWidth - Math.min(2, height) * 2}q{Math.min(2, height)} 0 {Math.min(
                    2,
                    height
                  )} {Math.min(2, height)}V{BASELINE}Z"
                />{/if}
              <text {x} y={BASELINE + 18} text-anchor="middle" class="weekday"
                >{day.date === today
                  ? m.stats_tab_today()
                  : weekday.format(localDate(day.date))}</text
              >
              <text {x} y={BASELINE + 32} text-anchor="middle" class="date-label"
                >{shortDate.format(localDate(day.date))}</text
              >
              {#if interaction.pinned === day.date}<line
                  class="selected-line"
                  x1={x - 9}
                  x2={x + 9}
                  y1={svgHeight - 3}
                  y2={svgHeight - 3}
                />{/if}
            </g>
          {/each}
          {#if summary?.rounds === 0}<text
              x={left + (width - left) / 2}
              y={12 + PLOT_HEIGHT * 0.4}
              text-anchor="middle"
              class="empty">{m.stats_week_empty()}</text
            >{/if}
        </svg>
      {:else}<div class="loading">{loading ? m.stats_loading() : '—'}</div>{/if}
    </div>
    <DateDetails
      date={pinned ? fullDate.format(localDate(pinned.date)) : null}
      lines={pinned ? details(pinned) : []}
      onclear={interaction.unpin}
      entryKey="week-day"
      onrecords={pinned && onopen
        ? () => {
            if (pinned) onopen?.({ date: pinned.date, focusKey: 'week-day' });
          }
        : undefined}
    />
  </div>
  {#if interaction.preview && preview}<StatsTooltip
      id={tooltipId}
      anchor={interaction.preview.anchor}
      lines={[fullDate.format(localDate(preview.date)), ...details(preview)]}
      onenter={interaction.keep}
      onleave={interaction.leave}
    />{/if}
</div>

<style>
  .view {
    min-width: 0;
    min-height: 100%;
    flex: 1 0 auto;
    display: flex;
    flex-direction: column;
  }
  .summary-row {
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-separator);
  }
  .chart-section {
    padding: var(--stats-block-gap, 20px) var(--stats-pad, 24px) 0;
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .toolbar,
  .context {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 16px;
  }
  h2 {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--color-foreground-darker);
  }
  .context {
    margin-top: 6px;
    font-size: 0.72rem;
    color: var(--color-foreground-darker);
  }
  .chart-wrap {
    margin: var(--stats-chart-gap, 16px) 0 8px;
    position: relative;
    flex: 1 0 190px;
    min-height: 190px;
    min-width: 0;
  }
  .chart {
    position: absolute;
    inset: 0;
    display: block;
    overflow: visible;
  }
  .gridline {
    stroke: var(--color-separator);
    stroke-width: 1;
  }
  .axis-label,
  .weekday,
  .date-label {
    fill: var(--color-foreground-darker);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }
  .weekday {
    font-size: 12px;
  }
  .date-label {
    opacity: 0.8;
  }
  g {
    cursor: pointer;
    outline: none;
  }
  .hit {
    fill: transparent;
    stroke: transparent;
    stroke-width: 2;
  }
  .bar {
    fill: color-mix(in oklch, var(--color-focus-round) 55%, var(--color-background-light));
    transition: fill 120ms;
    pointer-events: none;
  }
  .today .bar,
  g:hover .bar {
    fill: var(--color-focus-round);
  }
  .today .weekday {
    fill: var(--color-focus-round);
    font-weight: 600;
  }
  g:focus-visible .hit {
    stroke: color-mix(in oklch, var(--color-foreground) 45%, transparent);
  }
  .selected-line {
    stroke: var(--color-focus-round);
    stroke-width: 2;
  }
  .empty {
    font-size: 0.75rem;
    fill: var(--color-foreground-darker);
    pointer-events: none;
  }
  .loading {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--color-foreground-darker);
    font-size: 0.75rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
</style>
