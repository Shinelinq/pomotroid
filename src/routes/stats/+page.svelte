<script lang="ts">
  import '../../app.css';
  import '$lib/styles/auxiliary-scrollbars.css';
  import { onMount, tick } from 'svelte';
  import {
    getSettings,
    getThemes,
    onSettingsChanged,
    onThemesChanged,
    onRoundChange,
    onSessionsCleared,
    statsGetDetailed,
    statsGetHeatmap,
    auxiliaryWindowReady,
    onDataChanged,
    onCategoriesChanged,
    onStatsAll,
  } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import { applyTheme } from '$lib/stores/theme';
  import { setLocale } from '$lib/locale.svelte.js';
  import { resolveThemeName } from '$lib/utils/theme';
  import AuxiliaryWindowControls from '$lib/components/AuxiliaryWindowControls.svelte';
  import { isMac } from '$lib/utils/platform';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import type { DetailedStats, HeatmapStats, Theme, CategoryFilter as Filter } from '$lib/types';
  import CategoryFilter from '$lib/components/categories/CategoryFilter.svelte';
  import { categories } from '$lib/categories/state';
  import { filterKey, filterName } from '$lib/categories/format';
  import { createRequestScope } from '$lib/components/stats/requestScope';
  import * as m from '$paraglide/messages.js';
  import { error as logError } from '@tauri-apps/plugin-log';
  import DayDetail from '$lib/components/stats/DayDetail.svelte';
  import type { ChartSelection, DetailEntry } from '$lib/components/stats/sessionDetails';
  import DailyView from '$lib/components/stats/DailyView.svelte';
  import WeeklyView from '$lib/components/stats/WeeklyView.svelte';
  import YearlyView from '$lib/components/stats/YearlyView.svelte';
  import { dateKey, type Metric } from '$lib/components/stats/stats';
  import CategoryDistribution from '$lib/components/stats/CategoryDistribution.svelte';
  import ReportDialog from '$lib/components/data/ReportDialog.svelte';
  import { overviewScope } from '$lib/data/format';
  import type { ReportScope, Distribution, DistributionRow } from '$lib/data/types';

  type Tab = 'today' | 'week' | 'alltime';
  const tabs: Tab[] = ['today', 'week', 'alltime'];
  let activeTab = $state<Tab>('today');
  let weeklyMetric = $state<Metric>('time');
  let yearlyMetric = $state<Metric>('time');
  let selectedYear = $state(new Date().getFullYear());
  let report = $state<ReportScope | null>(null);
  let reportEntry: HTMLElement | null = null;
  let dataRevision = $state(0);
  const distributionCache = new Map<string, Distribution>();
  let weekExpanded = $state(false),
    yearExpanded = $state(false);
  let weekAll = $state(false),
    yearAll = $state(false);
  let distributionOrigin = $state<{ tab: Tab; name: string; scroll: number } | null>(null);
  function openReport(scope: ReportScope) {
    reportEntry = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    report = scope;
  }
  async function closeReport() {
    report = null;
    await tick();
    if (reportEntry?.isConnected) reportEntry.focus({ preventScroll: true });
  }
  async function selectDistribution(row: DistributionRow) {
    distributionOrigin = {
      tab: activeTab,
      name: row.name ?? m.category_uncategorized(),
      scroll: content?.scrollTop ?? 0,
    };
    changeFilter(
      row.category_id === null
        ? { kind: 'uncategorized' }
        : { kind: 'category', category_id: row.category_id },
      true
    );
    await tick();
    document
      .querySelector<HTMLElement>('[data-distribution-return]')
      ?.focus({ preventScroll: true });
  }
  async function backDistribution() {
    const scroll = distributionOrigin?.scroll ?? 0;
    changeFilter({ kind: 'all' }, true);
    distributionOrigin = null;
    await tick();
    document
      .querySelector<HTMLElement>('[data-distribution-toggle]')
      ?.focus({ preventScroll: true });
    if (content) content.scrollTop = scroll;
  }
  let dataRefresh: ReturnType<typeof setTimeout> | undefined;
  function invalidateData() {
    clearTimeout(dataRefresh);
    dataRefresh = setTimeout(() => {
      if (!disposed) {
        ++dataRevision;
        distributionCache.clear();
        refresh(true);
      }
    }, 30);
  }
  let today = $state(dateKey(new Date()));
  let detailedDate = $state(dateKey(new Date()));
  let detailed = $state<DetailedStats | null>(null);
  let heatmap = $state<HeatmapStats | null>(null);
  let detailedLoading = $state(false);
  let heatmapLoading = $state(false);
  let detailedError = $state(false);
  let heatmapError = $state(false);
  let initError = $state(false);
  let ready = false;
  let disposed = false;
  const requests = createRequestScope();
  let filter = $state<Filter>({ kind: 'all' });
  let filterEpoch = $state(0);
  const scopeName = $derived(filterName($categories?.data.items ?? [], filter));
  let detailedAt = 0;
  let heatmapAt = 0;
  let knownThemes: Theme[] = [];
  let content: HTMLDivElement;
  let detail = $state<{ date: string; hour: number | null; filter: Filter } | null>(null);
  let dailyView = $state<DailyView>();
  let weeklyView = $state<WeeklyView>();
  let yearlyView = $state<YearlyView>();
  let initialSelection = $state<ChartSelection | null>(null);
  type ReturnState = {
    tab: Tab;
    weeklyMetric: Metric;
    yearlyMetric: Metric;
    selectedYear: number;
    filter: Filter;
    selection: ChartSelection | null;
    scroll: number;
    focusKey: string;
  };
  let origin: ReturnState | null = null;
  let restoration = $state<{ scroll: number; focusKey: string; tab: Tab; token: number } | null>(
    null
  );
  let navigation = 0;
  function openDetail(entry: DetailEntry) {
    ++navigation;
    restoration = null;
    origin = {
      tab: activeTab,
      weeklyMetric,
      yearlyMetric,
      selectedYear,
      filter: { ...filter },
      selection:
        (activeTab === 'today'
          ? dailyView
          : activeTab === 'week'
            ? weeklyView
            : yearlyView
        )?.selection() ?? null,
      scroll: content?.scrollTop ?? 0,
      focusKey: entry.focusKey,
    };
    detail = { date: entry.date, hour: entry.hour ?? null, filter: { ...filter } };
    if (content) content.scrollTop = 0;
  }
  function leaveDetail(tab: Tab, restore = false) {
    const saved = restore ? origin : null;
    const token = ++navigation;
    activeTab = tab;
    initialSelection = saved?.selection ?? null;
    if (saved) {
      weeklyMetric = saved.weeklyMetric;
      yearlyMetric = saved.yearlyMetric;
      selectedYear = saved.selectedYear;
      filter = saved.filter;
    }
    // Clear visible aggregates before showing the overview; reload the real range.
    if (tab === 'alltime') {
      heatmap = null;
      heatmapAt = 0;
    } else {
      detailed = null;
      detailedAt = 0;
    }
    detail = null;
    origin = null;
    restoration = saved ? { scroll: saved.scroll, focusKey: saved.focusKey, tab, token } : null;
    if (content) content.scrollTop = 0;
    refresh(true);
  }
  const backToStats = () => leaveDetail(origin?.tab ?? activeTab, true);
  $effect(() => {
    const pending = restoration;
    if (!pending || detail || loading) return;
    let alive = true;
    void (async () => {
      await tick();
      await new Promise<void>((resolve) =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve()))
      );
      if (!alive || disposed || detail || pending.token !== navigation) return;
      if (content) content.scrollTop = pending.scroll;
      const entry = document.querySelector<HTMLElement>(
        `[data-detail-entry="${pending.focusKey}"]`
      );
      (entry ?? document.getElementById(`stats-tab-${pending.tab}`))?.focus({
        preventScroll: true,
      });
      restoration = null;
    })();
    return () => {
      alive = false;
    };
  });
  const STALE_MS = 30_000;
  const hasError = $derived(initError || (activeTab === 'alltime' ? heatmapError : detailedError));
  const loading = $derived(activeTab === 'alltime' ? heatmapLoading : detailedLoading);
  const reportError = (context: string, error: unknown) =>
    logError(`[stats] ${context}: ${error}`).catch(() => {});

  async function loadDetailed(force = false) {
    if (
      !force &&
      (detailedLoading ||
        (detailed &&
          !detailedError &&
          detailedDate === today &&
          Date.now() - detailedAt < STALE_MS))
    )
      return;
    const currentRequest = requests.begin('detailed');
    const requestedFilter = filter;
    const requestedDate = dateKey(new Date());
    detailedLoading = true;
    try {
      const result = await statsGetDetailed(requestedFilter);
      if (disposed || !currentRequest()) return;
      detailed = result;
      detailedDate = requestedDate;
      detailedAt = Date.now();
      detailedError = false;
    } catch (error) {
      if (!disposed && currentRequest()) detailedError = true;
      void reportError('detailed query failed', error);
    } finally {
      if (!disposed && currentRequest()) detailedLoading = false;
    }
  }
  async function loadHeatmap(force = false) {
    if (
      !force &&
      (heatmapLoading || (heatmap && !heatmapError && Date.now() - heatmapAt < STALE_MS))
    )
      return;
    const currentRequest = requests.begin('heatmap');
    const requestedFilter = filter;
    heatmapLoading = true;
    try {
      const result = await statsGetHeatmap(requestedFilter);
      if (disposed || !currentRequest()) return;
      heatmap = result;
      heatmapAt = Date.now();
      heatmapError = false;
    } catch (error) {
      if (!disposed && currentRequest()) heatmapError = true;
      void reportError('heatmap query failed', error);
    } finally {
      if (!disposed && currentRequest()) heatmapLoading = false;
    }
  }
  function refresh(force = false) {
    const nextDate = dateKey(new Date());
    const dayChanged = nextDate !== today;
    today = nextDate;
    if (!ready || disposed || detail) return;
    if (force || activeTab !== 'alltime' || dayChanged) void loadDetailed(force || dayChanged);
    if (activeTab === 'alltime' || ((force || dayChanged) && heatmap !== null))
      void loadHeatmap(force || dayChanged);
  }
  function changeFilter(next: Filter, fromDistribution = false) {
    if (!fromDistribution) distributionOrigin = null;
    if (detail || filterKey(next) === filterKey(filter)) return;
    initialSelection = null;
    restoration = null;
    ++navigation;
    filter = next;
    ++filterEpoch;
    requests.invalidate();
    detailed = null;
    heatmap = null;
    detailedAt = 0;
    heatmapAt = 0;
    detailedLoading = false;
    heatmapLoading = false;
    detailedError = false;
    heatmapError = false;
    refresh(true);
  }
  const noScopeData = $derived(
    filter.kind !== 'all' &&
      !loading &&
      !hasError &&
      (activeTab === 'alltime'
        ? heatmap !== null && heatmap.total_rounds === 0
        : activeTab === 'today'
          ? detailed !== null && detailed.today.completion_rate === null
          : detailed !== null && detailed.week.every((day) => day.started_rounds === 0))
  );
  function switchTab(tab: Tab) {
    if (detail) {
      leaveDetail(tab, tab === origin?.tab);
      return;
    }
    if (activeTab === tab) return;
    initialSelection = null;
    restoration = null;
    ++navigation;
    activeTab = tab;
    if (content) content.scrollTop = 0;
    refresh();
  }
  async function navigateTabs(event: KeyboardEvent) {
    const index = tabs.indexOf(activeTab);
    let next: Tab;
    if (event.key === 'ArrowLeft') next = tabs[(index + 2) % 3];
    else if (event.key === 'ArrowRight') next = tabs[(index + 1) % 3];
    else if (event.key === 'Home') next = tabs[0];
    else if (event.key === 'End') next = tabs[2];
    else return;
    event.preventDefault();
    switchTab(next);
    await tick();
    document.getElementById(`stats-tab-${next}`)?.focus();
  }
  function syncTheme() {
    const osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const theme =
      knownThemes.find((item) => item.name === resolveThemeName($settings, osDark)) ??
      knownThemes[0];
    if (theme) applyTheme(theme);
  }
  async function initialize() {
    initError = false;
    try {
      const [saved, themes] = await Promise.all([getSettings(), getThemes()]);
      if (disposed) return;
      settings.set(saved);
      knownThemes = themes;
      setLocale(saved.language);
      syncTheme();
      ready = true;
      refresh(true);
    } catch (error) {
      if (!disposed) initError = true;
      void reportError('initialization failed', error);
    } finally {
      if (!disposed) auxiliaryWindowReady().catch((error) => reportError('show failed', error));
    }
  }
  function retry() {
    if (initError) void initialize();
    else refresh(true);
  }

  onMount(() => {
    const cleanups: UnlistenFn[] = [];
    const escapeDetail = (event: KeyboardEvent) => {
      if (!detail || event.key !== 'Escape' || event.defaultPrevented) return;
      if (document.querySelector('[popover]:popover-open, dialog[open], [role="tooltip"]')) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      backToStats();
    };
    document.addEventListener('keydown', escapeDetail, true);
    cleanups.push(() => document.removeEventListener('keydown', escapeDetail, true));
    async function keep(promise: Promise<UnlistenFn>) {
      try {
        const unlisten = await promise;
        if (disposed) unlisten();
        else cleanups.push(unlisten);
      } catch (error) {
        void reportError('event subscription failed', error);
      }
    }
    void keep(onRoundChange(invalidateData));
    void keep(onDataChanged(invalidateData));
    void keep(onCategoriesChanged(invalidateData));
    void keep(
      onStatsAll(() => {
        if (detail) leaveDetail(activeTab);
        changeFilter({ kind: 'all' });
        refresh(true);
      })
    );
    void keep(onSessionsCleared(() => refresh(true)));
    void keep(
      onSettingsChanged((updated) => {
        settings.set(updated);
        setLocale(updated.language);
        syncTheme();
      })
    );
    void keep(
      onThemesChanged((themes) => {
        knownThemes = themes;
        syncTheme();
      })
    );
    const onFocus = () => refresh();
    const onVisibility = () => {
      if (!document.hidden) refresh();
    };
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    mq.addEventListener('change', syncTheme);
    window.addEventListener('focus', onFocus);
    document.addEventListener('visibilitychange', onVisibility);
    let midnightTimer: ReturnType<typeof setTimeout>;
    function scheduleMidnight() {
      const now = new Date();
      const midnight = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
      midnightTimer = setTimeout(
        () => {
          refresh(true);
          scheduleMidnight();
        },
        midnight.getTime() - now.getTime() + 50
      );
    }
    scheduleMidnight();
    void initialize();
    return () => {
      disposed = true;
      requests.invalidate();
      clearTimeout(midnightTimer);
      clearTimeout(dataRefresh);
      for (const unlisten of cleanups) unlisten();
      mq.removeEventListener('change', syncTheme);
      window.removeEventListener('focus', onFocus);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  });
</script>

<div class="window">
  <!-- Titlebar -->
  <nav class="titlebar" class:macos={isMac} data-tauri-drag-region>
    <span class="titlebar-label">{m.stats_title()}</span>
    {#if !isMac}
      <AuxiliaryWindowControls />
    {/if}
  </nav>

  <!-- Existing tab styling is retained; only keyboard and tab semantics are added. -->
  <div class="stats-navigation">
    <div class="tabs" role="tablist" aria-label={m.stats_title()}>
      {#each tabs as tab}
        <button
          id="stats-tab-{tab}"
          class="tab"
          class:active={activeTab === tab}
          role="tab"
          aria-selected={activeTab === tab}
          aria-controls="stats-panel"
          tabindex={activeTab === tab ? 0 : -1}
          onclick={() => switchTab(tab)}
          onkeydown={navigateTabs}
          >{tab === 'today'
            ? m.stats_tab_today()
            : tab === 'week'
              ? m.stats_tab_week()
              : m.stats_tab_alltime()}</button
        >
      {/each}
    </div>
    {#if !detail}<div class="category-filter">
        <CategoryFilter value={filter} onchange={(next) => changeFilter(next)} />
        <button
          class="export-button"
          title={m.report_title()}
          aria-label={m.report_title()}
          onclick={() =>
            openReport({ ...overviewScope(activeTab, today, selectedYear), filter: { ...filter } })}
          >⇩</button
        >
      </div>{/if}
  </div>

  <div
    bind:this={content}
    class="content aux-scroll"
    class:detail-content={!!detail}
    id="stats-panel"
    role="tabpanel"
    aria-labelledby="stats-tab-{activeTab}"
    aria-busy={!detail && loading}
  >
    {#if detail}
      <DayDetail
        initialDate={detail.date}
        initialHour={detail.hour}
        filter={detail.filter}
        {today}
        onback={backToStats}
        onexport={openReport}
      />
    {:else}
      {#if hasError}<div class="load-error" role="status">
          <span>{m.stats_load_error()}</span><button onclick={retry} disabled={loading}
            >{m.stats_retry()}</button
          >
        </div>{/if}
      {#if noScopeData}<div class="scope-empty">
          <span>{m.category_no_data()}</span><button onclick={() => changeFilter({ kind: 'all' })}
            >{m.category_view_all()}</button
          >
        </div>{/if}
      {#key filterEpoch}
        {#if activeTab === 'today'}
          {#if detailed}<DailyView
              bind:this={dailyView}
              today={detailed.today}
              date={detailedDate}
              {initialSelection}
              onopen={openDetail}
            />{:else}<div class="initial-loading">
              {detailedLoading ? m.stats_loading() : '—'}
            </div>{/if}
        {:else if activeTab === 'week'}
          <WeeklyView
            bind:this={weeklyView}
            {initialSelection}
            onopen={openDetail}
            week={detailed?.week ?? null}
            streak={detailed?.streak ?? null}
            today={detailed ? detailedDate : today}
            loading={detailedLoading}
            bind:metric={weeklyMetric}
            distribution={distributionSlot}
          />
        {:else}
          <YearlyView
            bind:this={yearlyView}
            {initialSelection}
            onopen={openDetail}
            {heatmap}
            {today}
            loading={heatmapLoading}
            bind:metric={yearlyMetric}
            bind:selectedYear
            scopeLabel={filter.kind === 'all' ? null : scopeName}
            distribution={distributionSlot}
          />
        {/if}
      {/key}
    {/if}
  </div>
</div>

{#snippet distributionSlot()}
  {@const range = overviewScope(activeTab, today, selectedYear)}
  {#if activeTab === 'week'}
    <CategoryDistribution
      start={range.start!}
      end={range.end!}
      label={m.distribution_recent()}
      visible={filter.kind === 'all'}
      returnName={distributionOrigin?.tab === activeTab ? distributionOrigin.name : null}
      bind:expanded={weekExpanded}
      bind:all={weekAll}
      revision={dataRevision}
      cache={distributionCache}
      onselect={selectDistribution}
      onback={backDistribution}
    />
  {:else if activeTab === 'alltime'}
    <CategoryDistribution
      start={range.start!}
      end={range.end!}
      label={m.distribution_year({ year: selectedYear })}
      visible={filter.kind === 'all'}
      returnName={distributionOrigin?.tab === activeTab ? distributionOrigin.name : null}
      bind:expanded={yearExpanded}
      bind:all={yearAll}
      revision={dataRevision}
      cache={distributionCache}
      onselect={selectDistribution}
      onback={backDistribution}
    />
  {/if}
{/snippet}
{#if report}<ReportDialog source={report} onclose={closeReport} />{/if}

<style>
  .export-button {
    width: 28px;
    height: 28px;
    flex: none;
    margin-left: 8px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground);
    font-size: 18px;
    cursor: pointer;
  }
  .export-button:hover {
    background: var(--color-hover);
  }
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-background);
    color: var(--color-foreground);
    overflow: hidden;
    cursor: default;
  }

  /* ── Titlebar ──────────────────────────────────────────── */
  .titlebar {
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-separator);
  }

  .macos {
    padding-left: 72px;
  }

  .titlebar-label {
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--color-foreground-darker);
    pointer-events: none;
  }

  /* ── Tabs ──────────────────────────────────────────────── */
  .tabs {
    display: flex;
    gap: 0;
    flex-shrink: 0;
    padding: 0 24px;
  }

  .stats-navigation {
    display: flex;
    flex-wrap: wrap;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-separator);
  }
  .category-filter {
    flex: 0 0 200px;
    min-width: 0;
    max-width: 100%;
    height: 28px;
    align-self: center;
    margin-left: auto;
    padding-right: 24px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }
  .scope-empty {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 24px;
    font-size: 11px;
    color: var(--color-foreground-darker);
  }
  .scope-empty button {
    min-height: 28px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground);
    font: inherit;
    padding: 4px 6px;
    cursor: pointer;
  }
  .scope-empty button:hover {
    background: var(--color-hover);
  }

  .tab {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
    padding: 10px 20px;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-foreground-darker);
    cursor: pointer;
    transition:
      color 0.15s,
      border-color 0.15s;
  }

  .tab:hover {
    color: var(--color-foreground);
  }

  .tab.active {
    color: var(--color-focus-round);
    border-bottom-color: var(--color-focus-round);
  }

  /* ── Content ───────────────────────────────────────────── */
  .content {
    flex: 1;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    display: flex;
    flex-direction: column;
    --stats-pad: 24px;
    --stats-block-gap: 20px;
    --stats-chart-gap: 16px;
  }
  @media (max-height: 560px) {
    .content {
      --stats-pad: 16px;
      --stats-block-gap: 12px;
      --stats-chart-gap: 10px;
      --stats-summary-y: 14px;
      --stats-summary-min: 88px;
      --stats-summary-gap: 4px;
      --stats-number-size: 1.75rem;
      --stats-detail-min: 40px;
      --stats-detail-y: 4px;
    }
  }
  .content.detail-content {
    overflow: hidden;
  }
  .load-error {
    flex-shrink: 0;
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 8px 24px;
    color: var(--color-foreground-darker);
    font-size: 0.75rem;
    border-bottom: 1px solid var(--color-separator);
  }
  .load-error button {
    font: inherit;
    border: 0;
    background: transparent;
    color: var(--color-focus-round);
    text-decoration: underline;
    cursor: pointer;
  }
  .load-error button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .initial-loading {
    padding: 24px;
    color: var(--color-foreground-darker);
    font-size: 0.75rem;
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    .tab {
      transition: none;
    }
  }
</style>
