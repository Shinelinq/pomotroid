<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import type { CategoryFilter, SessionRecord, TimerState } from '$lib/types';
  import {
    statsGetSessions,
    getTimerState,
    onTimerState,
    onSessionsCleared,
    onDataChanged,
  } from '$lib/ipc';
  import type { ReportScope } from '$lib/data/types';
  import { categories, connectCategories } from '$lib/categories/state';
  import { filterName } from '$lib/categories/format';
  import { getLocale } from '$paraglide/runtime.js';
  import { error as logError } from '@tauri-apps/plugin-log';
  import * as m from '$paraglide/messages.js';
  import { hourRange } from './stats';
  import {
    calendarDate,
    shiftDetailDate,
    exactDuration,
    sessionStatus,
    sessionTimes,
  } from './sessionDetails';
  import { createSessionList, emptySessionList } from './sessionList';

  let {
    initialDate,
    initialHour = null,
    filter,
    today,
    onback,
    onexport,
  }: {
    initialDate: string;
    initialHour?: number | null;
    filter: CategoryFilter;
    today: string;
    onback: () => void;
    onexport?: (scope: ReportScope) => void;
  } = $props();
  let date = $state(untrack(() => initialDate));
  let hour = $state<number | null>(untrack(() => initialHour));
  let view = $state(emptySessionList());
  let runtime = $state<TimerState | null>(null);
  let controller = $state<ReturnType<typeof createSessionList>>();
  let initializing = $state(true);
  let initError = $state(false);
  let scrollArea: HTMLDivElement;
  let dateHeading: HTMLHeadingElement;
  let backButton: HTMLButtonElement;
  let moreButton = $state<HTMLButtonElement>();
  let disposed = false;
  let refreshQueued = false;
  let cleanupListeners: (() => void)[] = [];
  const previous = $derived(shiftDetailDate(date, -1));
  const next = $derived(shiftDetailDate(date, 1));
  const number = $derived(new Intl.NumberFormat(getLocale()));
  const dateTitle = $derived(
    new Intl.DateTimeFormat(getLocale(), { dateStyle: 'full', timeZone: 'UTC' }).format(
      calendarDate(date)
    )
  );
  const scopeLabel = $derived(filterName($categories?.data.items ?? [], filter));
  const statusText = (row: SessionRecord) =>
    ({
      completed: m.detail_completed,
      incomplete: m.detail_incomplete,
      running: m.detail_running,
      paused: m.detail_paused,
    })[sessionStatus(row, runtime)]();
  function category(row: SessionRecord) {
    const latest = $categories?.data.items.find((item) => item.id === row.category_id);
    return {
      name:
        row.category_id === null
          ? m.category_uncategorized()
          : (latest?.name ?? row.category_name ?? m.category_unknown()),
      archived: latest?.archived ?? row.category_archived,
    };
  }
  function timeRange(row: SessionRecord) {
    const time = sessionTimes(row, getLocale());
    if (time.end === null) return `${time.start} —`;
    const end = time.nextDay
      ? m.detail_next_day({ time: time.end })
      : time.endDate
        ? `${time.endDate} ${time.end}`
        : time.end;
    return `${time.start} — ${end}`;
  }
  const report = (error: unknown) => logError(`[stats/detail] ${error}`).catch(() => {});

  function scheduleRefresh() {
    if (!controller || disposed || refreshQueued) return;
    refreshQueued = true;
    queueMicrotask(() => {
      refreshQueued = false;
      if (!disposed) void controller?.refresh();
    });
  }
  function receiveTimer(value: TimerState) {
    if (disposed || (runtime && value.revision < runtime.revision)) return;
    const previous = runtime;
    runtime = value;
    // Pause/resume only changes the status text. First write and completion may
    // change membership/summary; ordinary one-second ticks never query the page.
    if (previous && previous.session_id !== value.session_id) {
      // The active row may be on an unloaded page or may have started yesterday.
      scheduleRefresh();
    }
  }
  async function initialize() {
    initializing = true;
    initError = false;
    cleanupListeners.splice(0).forEach((stop) => stop());
    let alive = true;
    async function keep(promise: Promise<() => void>) {
      const stop = await promise;
      if (disposed || !alive) stop();
      else cleanupListeners.push(stop);
    }
    try {
      await Promise.all([
        keep(onTimerState(receiveTimer)),
        keep(onSessionsCleared(scheduleRefresh)),
        keep(onDataChanged(scheduleRefresh)),
      ]);
      const initial = await getTimerState();
      if (disposed) return;
      receiveTimer(initial);
      if (controller) void controller.refresh();
      else {
        controller = createSessionList(statsGetSessions, (value) => {
          if (!disposed) view = value;
        });
        void controller.open({ date, hour, filter });
      }
    } catch (error) {
      alive = false;
      cleanupListeners.splice(0).forEach((stop) => stop());
      if (!disposed) initError = true;
      void report(error);
    } finally {
      if (!disposed) initializing = false;
    }
  }
  async function navigate(value: string) {
    if (value > today) return;
    date = value;
    hour = null;
    view = emptySessionList();
    void controller?.open({ date, hour, filter });
    if (scrollArea) scrollArea.scrollTop = 0;
    await tick();
    dateHeading?.focus({ preventScroll: true });
  }
  async function wholeDay() {
    hour = null;
    view = emptySessionList();
    void controller?.open({ date, hour, filter });
    if (scrollArea) scrollArea.scrollTop = 0;
    await tick();
    dateHeading?.focus({ preventScroll: true });
  }
  async function more() {
    const origin = document.activeElement;
    await controller?.more();
    await tick();
    if (document.activeElement === origin || document.activeElement === document.body) {
      (moreButton ?? scrollArea)?.focus({ preventScroll: true });
    }
  }
  function retry() {
    if (initError) void initialize();
    else void controller?.retry();
  }
  onMount(() => {
    const catalog = connectCategories(false);
    void initialize();
    void tick().then(() => {
      if (!disposed) backButton?.focus({ preventScroll: true });
    });
    const focus = () => {
      if (disposed || initializing) return;
      if (initError) {
        void initialize();
        return;
      }
      void getTimerState()
        .then(receiveTimer)
        .catch((error) => {
          if (!disposed) initError = true;
          void report(error);
        });
      scheduleRefresh();
    };
    const visibility = () => {
      if (!document.hidden) focus();
    };
    window.addEventListener('focus', focus);
    document.addEventListener('visibilitychange', visibility);
    return () => {
      disposed = true;
      controller?.dispose();
      catalog.dispose();
      cleanupListeners.splice(0).forEach((stop) => stop());
      window.removeEventListener('focus', focus);
      document.removeEventListener('visibilitychange', visibility);
    };
  });
</script>

<section class="day-detail" aria-label={m.detail_title()}>
  <div class="detail-header">
    <div class="date-navigation">
      <button bind:this={backButton} class="text-button" onclick={onback}>{m.detail_back()}</button>
      <h2 bind:this={dateHeading} tabindex="-1">{dateTitle}</h2>
      <div class="date-actions">
        {#if onexport}<button
            class="icon-button"
            title={m.report_title()}
            aria-label={m.report_title()}
            onclick={() =>
              onexport?.({ kind: 'sessions', start: date, end: date, filter: { ...filter }, hour })}
            >⇩</button
          >{/if}
        {#if date !== today}<button class="text-button" onclick={() => navigate(today)}
            >{m.detail_today()}</button
          >{/if}
        <button
          class="icon-button"
          aria-label={m.detail_previous()}
          title={m.detail_previous()}
          disabled={!previous}
          onclick={() => previous && navigate(previous)}
          ><svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"
            ><path d="m9 3-4 4 4 4" stroke="currentColor" stroke-width="1.5" fill="none" /></svg
          ></button
        >
        <button
          class="icon-button"
          aria-label={m.detail_next()}
          title={m.detail_next()}
          disabled={!next || date >= today}
          onclick={() => next && navigate(next)}
          ><svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true"
            ><path d="m5 3 4 4-4 4" stroke="currentColor" stroke-width="1.5" fill="none" /></svg
          ></button
        >
      </div>
    </div>
    <div class="scope-line">
      <span
        >{filter.kind === 'all' ? scopeLabel : m.detail_category_scope({ name: scopeLabel })}</span
      >
      {#if hour !== null}<span>{m.stats_hour_range(hourRange(hour))}</span><button
          class="text-button"
          onclick={wholeDay}>{m.detail_whole_day()}</button
        >{/if}
    </div>
    <p class="summary" title={view.summary ? exactDuration(view.summary.focus_secs) : undefined}>
      {view.summary
        ? m.detail_summary({
            completed: number.format(view.summary.completed),
            recorded: number.format(view.summary.recorded),
            duration: exactDuration(view.summary.focus_secs),
          })
        : '—'}
    </p>
    <p class="explanation">{m.detail_time_help()}</p>
    {#if initError || view.error}<div class="load-error" role="alert">
        <span>{m.detail_load_error()}</span><button
          class="text-button"
          disabled={initializing || view.loading || view.loadingMore}
          onclick={retry}>{m.stats_retry()}</button
        >
      </div>{/if}
    {#if initializing || view.loading}<p class="loading" role="status">
        {view.summary ? m.detail_refreshing() : m.stats_loading()}
      </p>{/if}
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (A read-only scroll region needs keyboard focus for Arrow/PageDown scrolling.) -->
  <div
    class="record-scroll aux-scroll"
    bind:this={scrollArea}
    tabindex="0"
    role="region"
    aria-label={m.detail_records()}
    aria-busy={initializing || view.loading || view.loadingMore}
  >
    {#if view.summary && view.records.length > 0}
      <table>
        <colgroup
          ><col class="time-col" /><col class="category-col" /><col class="duration-col" /><col
            class="status-col"
          /></colgroup
        >
        <thead
          ><tr
            ><th scope="col">{m.detail_time_column()}</th><th scope="col">{m.category_title()}</th
            ><th scope="col">{m.detail_duration_column()}</th><th scope="col"
              >{m.detail_status_column()}</th
            ></tr
          ></thead
        >
        <tbody
          >{#each view.records as row (row.id)}
            {@const classification = category(row)}
            <tr data-session-id={row.id}>
              <td class="time" title={sessionTimes(row, getLocale()).title}>{timeRange(row)}</td>
              <td class="classification"
                ><span title={classification.name}>{classification.name}</span
                >{#if classification.archived}<small>{m.category_archived()}</small>{/if}</td
              >
              <td class="duration" data-label={m.detail_duration_column()}
                >{row.completed
                  ? exactDuration(row.duration_secs)
                  : m.detail_planned({ duration: exactDuration(row.duration_secs) })}</td
              >
              <td class="status" data-label={m.detail_status_column()}>{statusText(row)}</td>
            </tr>
          {/each}</tbody
        >
      </table>
    {:else if view.summary?.recorded === 0 && !view.loading && !view.error && !initError}
      <div class="empty">
        <p>{m.detail_empty()}</p>
        {#if hour !== null}<button class="text-button" onclick={wholeDay}
            >{m.detail_whole_day()}</button
          >{/if}
      </div>
    {/if}
    {#if view.cursor}<div class="more">
        <button
          bind:this={moreButton}
          class="text-button"
          disabled={view.loading || view.loadingMore}
          onclick={more}>{view.loadingMore ? m.stats_loading() : m.detail_more()}</button
        >
      </div>{/if}
  </div>
</section>

<style>
  .day-detail {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    min-width: 0;
    font-size: 12px;
    color: var(--color-foreground);
  }
  .detail-header {
    flex-shrink: 0;
    padding: 10px var(--stats-pad, 24px) 8px;
    border-bottom: 1px solid var(--color-separator);
  }
  .date-navigation {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 8px;
  }
  h2 {
    flex: 1;
    text-align: center;
    font-size: 13px;
    font-weight: 500;
    min-width: 130px;
    margin: 0;
  }
  .date-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .text-button,
  .icon-button {
    background: transparent;
    border: 0;
    border-radius: 4px;
    color: var(--color-foreground-darker);
    font: inherit;
    cursor: pointer;
    min-height: 28px;
    padding: 4px 6px;
  }
  .icon-button {
    width: 28px;
    height: 28px;
    padding: 0;
    display: grid;
    place-items: center;
    flex: none;
  }
  button:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button:focus-visible,
  h2:focus-visible,
  .record-scroll:focus-visible {
    outline: 1px solid var(--color-foreground);
    outline-offset: -1px;
  }
  .scope-line {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px 12px;
    min-height: 28px;
    color: var(--color-foreground-darker);
  }
  .scope-line > span {
    overflow-wrap: anywhere;
  }
  .summary {
    margin-top: 6px;
    line-height: 1.6;
    font-variant-numeric: tabular-nums;
  }
  .explanation {
    margin-top: 4px;
    font-size: 11px;
    line-height: 1.5;
    color: var(--color-foreground-darker);
  }
  .loading {
    font-size: 11px;
    margin-top: 4px;
    color: var(--color-foreground-darker);
  }
  .load-error {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    font-size: 11px;
  }
  .record-scroll {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 var(--stats-pad, 24px);
  }
  table {
    border-collapse: collapse;
    table-layout: fixed;
    width: 100%;
    text-align: left;
  }
  .time-col {
    width: 32%;
  }
  .category-col {
    width: 26%;
  }
  .duration-col {
    width: 24%;
  }
  .status-col {
    width: 18%;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--color-background);
    color: var(--color-foreground-darker);
    font-size: 11px;
    font-weight: 500;
    padding: 8px 6px;
    border-bottom: 1px solid var(--color-separator);
  }
  td {
    height: 52px;
    padding: 8px 6px;
    border-bottom: 1px solid var(--color-separator);
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  tbody tr:hover {
    background: color-mix(in oklch, var(--color-hover) 50%, transparent);
  }
  .time,
  .duration {
    font-variant-numeric: tabular-nums;
  }
  .classification > span {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small {
    color: var(--color-foreground-darker);
    font-size: 11px;
  }
  .empty {
    padding: 24px 0;
    color: var(--color-foreground-darker);
    line-height: 1.6;
  }
  .more {
    padding: 8px 0 12px;
    text-align: center;
  }
  @media (max-width: 560px) {
    .date-navigation h2 {
      order: 3;
      flex-basis: 100%;
      padding: 4px 0;
    }
    .date-actions {
      margin-left: auto;
    }
    thead {
      position: absolute;
      width: 1px;
      height: 1px;
      clip-path: inset(50%);
      overflow: hidden;
    }
    colgroup {
      display: none;
    }
    table,
    tbody {
      display: block;
    }
    tbody tr {
      display: grid;
      grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
      column-gap: 10px;
      row-gap: 4px;
      padding: 8px 0;
      border-bottom: 1px solid var(--color-separator);
      min-height: 54px;
    }
    td {
      display: block;
      height: auto;
      padding: 0;
      border: 0;
    }
    .duration::before,
    .status::before {
      content: attr(data-label);
      margin-right: 6px;
      color: var(--color-foreground-darker);
      font-size: 11px;
    }
    .classification {
      display: flex;
      align-items: baseline;
      gap: 4px;
    }
    .classification small {
      flex: none;
    }
    .classification > span {
      display: block;
      white-space: nowrap;
      min-width: 0;
      flex: 1;
    }
  }
</style>
