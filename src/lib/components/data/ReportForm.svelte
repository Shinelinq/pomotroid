<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import { dataBegin, dataCancel, reportPreview, reportSave } from '$lib/ipc';
  import type { ReportScope, ReportInfo, Saved } from '$lib/data/types';
  import { dataError, reportDates } from '$lib/data/format';
  import { hourRange } from '$lib/components/stats/stats';
  import CategoryFilter from '$lib/components/categories/CategoryFilter.svelte';
  import { categories } from '$lib/categories/state';
  import { filterName } from '$lib/categories/format';
  import '$lib/data/data.css';
  let {
    source = null,
    onclose,
    onbusy = () => {},
  }: {
    source?: ReportScope | null;
    onclose: () => void;
    onbusy?: (busy: boolean) => void;
  } = $props();
  let scope = $state<ReportScope>(
    untrack(() =>
      source
        ? { ...source, filter: { ...source.filter } }
        : { kind: 'daily', ...reportDates('recent'), filter: { kind: 'all' }, hour: null }
    )
  );
  let preset = $state(untrack(() => (source ? 'current' : 'recent')));
  let loading = $state(false);
  let saving = $state(false);
  let error = $state('');
  let notice = $state('');
  let info = $state<ReportInfo | null>(null);
  let saved = $state<Saved | null>(null);
  let token = '';
  let generation = 0;
  let disposed = false;
  const categoryName = $derived(filterName($categories?.data.items ?? [], scope.filter));
  function changeRange(value: string) {
    preset = value;
    if (value === 'current' && source) {
      scope = {
        ...source,
        filter: { ...scope.filter },
        kind: source.hour !== null ? 'sessions' : scope.kind,
      };
      notice = '';
      return;
    }
    if (scope.hour !== null) notice = m.report_hour_cleared();
    scope = {
      ...scope,
      hour: null,
      ...(value === 'custom' ? (scope.start ? {} : reportDates('recent')) : reportDates(value)),
    };
  }
  $effect(() => {
    const requested = JSON.stringify(scope);
    const locale = getLocale();
    if (saved) return;
    const current = ++generation;
    info = null;
    error = '';
    loading = true;
    const timer = setTimeout(async () => {
      let own = '';
      try {
        own = await dataBegin();
        if (disposed || current !== generation) {
          void dataCancel(own);
          return;
        }
        token = own;
        const result = await reportPreview(own, JSON.parse(requested), locale);
        if (!disposed && current === generation) info = result;
      } catch (e) {
        if (!disposed && current === generation) error = dataError(e);
      } finally {
        if (!disposed && current === generation) loading = false;
      }
    }, 150);
    return () => {
      clearTimeout(timer);
    };
  });
  async function save() {
    if (!info || loading || saving) return;
    saving = true;
    onbusy(true);
    error = '';
    try {
      saved = await reportSave(token);
      if (saved) token = '';
    } catch (e) {
      error = dataError(e);
    } finally {
      saving = false;
      onbusy(false);
    }
  }
  onMount(() => () => {
    disposed = true;
    ++generation;
    if (token) void dataCancel(token);
  });
</script>

<div class="data-flow">
  <h2>{m.data_csv()}</h2>
  {#if saved}
    <p role="status">{m.report_saved({ rows: saved.rows, filename: saved.filename })}</p>
    <div class="actions"><button onclick={onclose}>{m.data_done()}</button></div>
  {:else}
    <div class="fields">
      <label
        >{m.report_type()}<select bind:value={scope.kind} disabled={saving || scope.hour !== null}
          ><option value="daily">{m.report_daily()}</option><option value="sessions"
            >{m.report_sessions()}</option
          ></select
        ></label
      >
      <label
        >{m.report_range()}<select
          value={preset}
          disabled={saving}
          onchange={(e) => changeRange(e.currentTarget.value)}
        >
          {#if source}<option value="current">{m.report_current()}</option>{/if}
          <option value="recent">{m.report_recent()}</option><option value="month"
            >{m.report_month()}</option
          ><option value="previous">{m.report_previous_month()}</option><option value="all"
            >{m.data_all_history()}</option
          ><option value="custom">{m.report_custom()}</option>
        </select></label
      >
      {#if preset === 'custom'}
        <div class="dates">
          <label
            >{m.report_start()}<input
              type="date"
              bind:value={scope.start}
              disabled={saving}
            /></label
          ><label
            >{m.report_end()}<input type="date" bind:value={scope.end} disabled={saving} /></label
          >
        </div>
      {/if}
      <div class="filter">
        <span>{m.category_filter()}</span><CategoryFilter
          value={scope.filter}
          onchange={(filter) => {
            if (!saving) scope = { ...scope, filter };
          }}
        />
      </div>
    </div>
    {#if scope.hour !== null}
      <p>
        {m.report_hour({ hour: hourRange(scope.hour) })}
        <button
          class="text-button"
          disabled={saving}
          onclick={() => (scope = { ...scope, hour: null })}>{m.report_whole_day()}</button
        >
      </p>
      <p class="muted">{m.report_hour_type()}</p>
    {/if}
    {#if notice}<p class="muted" role="status">{notice}</p>{/if}
    <hr />
    <p>
      {scope.kind === 'daily' ? m.report_daily() : m.report_sessions()} · {scope.start ??
        m.data_all_history()}{scope.end ? ` — ${scope.end}` : ''} · {categoryName}{scope.hour !==
      null
        ? ` · ${hourRange(scope.hour)}`
        : ''}
    </p>
    <p class="muted">{m.report_local_zone({ zone: info?.timezone ?? m.data_unknown() })}</p>
    <div aria-live="polite">
      {#if loading}<p>{m.data_validating()}</p>{:else if info}<p>
          {m.report_summary({ rows: info.rows, seconds: info.focus_secs })}
        </p>
        {#if info.rows === 0}<p>{m.report_empty()}</p>{/if}{/if}
      {#if error}<p class="notice" role="alert">{error}</p>{/if}
      {#if saving}<p>{m.data_saving()}</p>{/if}
    </div>
    <p class="muted">{m.report_formula_note()}</p>
    <div class="actions">
      <button disabled={saving} onclick={onclose}>{m.data_cancel()}</button><button
        class="primary"
        disabled={!info || loading || saving}
        onclick={save}>{m.report_save()}</button
      >
    </div>
  {/if}
</div>

<style>
  .fields {
    display: grid;
    gap: 12px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .dates {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: 12px;
  }
  .filter {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  @media (max-width: 400px) {
    .dates {
      grid-template-columns: minmax(0, 1fr);
    }
  }
</style>
