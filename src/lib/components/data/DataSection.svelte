<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import * as m from '$paraglide/messages.js';
  import { getLocale } from '$paraglide/runtime.js';
  import {
    dataBegin,
    dataCancel,
    dataRead,
    dataReplan,
    dataDetails,
    dataCommit,
    dataExport,
    dataLastExport,
    onDataPhase,
    getTimerState,
    onTimerState,
    dataViewStats,
  } from '$lib/ipc';
  import type {
    ImportPreview,
    ImportOptions,
    ImportSummary,
    Conflict,
    Rename,
    Saved,
  } from '$lib/data/types';
  import { dataError } from '$lib/data/format';
  import ReportForm from './ReportForm.svelte';
  import '$lib/data/data.css';
  let { busy = $bindable(false) }: { busy?: boolean } = $props();
  let page = $state<'home' | 'export' | 'import' | 'report' | 'exported' | 'imported'>('home');
  let profiles = $state(true);
  let preferences = $state(false);
  let last = $state<string | null>(null);
  let preview = $state<ImportPreview | null>(null);
  let pending = $state(false);
  let phase = $state('reading');
  let error = $state('');
  let notice = $state('');
  let activeRound = $state(false);
  let conflicts = $state<Conflict[]>([]);
  let renames = $state<Rename[]>([]);
  let showConflicts = $state(false);
  let showRenames = $state(false);
  let detailsLoading = $state(false);
  let saved = $state<Saved | null>(null);
  let imported = $state<ImportSummary | null>(null);
  let token = '';
  let epoch = 0;
  let disposed = false;
  let entry = 'data-export-entry';
  let homeScroll = 0;
  const additions = $derived(
    preview
      ? preview.summary.sessions.added +
          preview.summary.categories.added +
          preview.summary.profiles.added +
          (preview.summary.preferences ? 1 : 0)
      : 0
  );
  const date = (v: string | number | null) =>
    v === null
      ? '—'
      : new Intl.DateTimeFormat(getLocale(), { dateStyle: 'medium', timeStyle: 'short' }).format(
          new Date(typeof v === 'number' ? v * 1000 : v)
        );
  const kind = (s: string) =>
    s === 'session'
      ? m.data_sessions()
      : s === 'category'
        ? m.data_categories()
        : m.data_profiles();
  function enter(next: typeof page, id: string) {
    homeScroll = document.querySelector('.content')?.scrollTop ?? 0;
    entry = id;
    page = next;
    error = '';
    notice = '';
    void tick().then(() => {
      const c = document.querySelector('.content');
      if (c) c.scrollTop = 0;
      document.getElementById('data-heading')?.focus();
    });
  }
  async function back() {
    if (busy) return;
    ++epoch;
    if (token) {
      void dataCancel(token);
      token = '';
    }
    pending = false;
    preview = null;
    page = 'home';
    error = '';
    notice = '';
    await tick();
    const c = document.querySelector('.content');
    if (c) c.scrollTop = homeScroll;
    document.getElementById(entry)?.focus({ preventScroll: true });
  }
  async function choose() {
    enter('import', 'data-import-entry');
    const current = ++epoch;
    pending = true;
    phase = 'reading';
    conflicts = [];
    renames = [];
    showConflicts = false;
    showRenames = false;
    try {
      const own = await dataBegin();
      if (disposed || current !== epoch) {
        void dataCancel(own);
        return;
      }
      token = own;
      const result = await dataRead(own, getLocale());
      if (disposed || current !== epoch) return;
      preview = result;
      if (!result) await back();
    } catch (e) {
      if (!disposed && current === epoch && String(e) !== 'data_cancelled') error = dataError(e);
    } finally {
      if (!disposed && current === epoch) pending = false;
    }
  }
  async function options(key: keyof ImportOptions, value: boolean) {
    if (!preview || pending) return;
    const current = epoch;
    pending = true;
    phase = 'validating';
    error = '';
    conflicts = [];
    renames = [];
    showConflicts = false;
    showRenames = false;
    try {
      const result = await dataReplan(token, { ...preview.options, [key]: value });
      if (!disposed && current === epoch) preview = result;
    } catch (e) {
      if (!disposed && current === epoch) error = dataError(e);
    } finally {
      if (!disposed && current === epoch) pending = false;
    }
  }
  async function detail(renamed: boolean) {
    if (detailsLoading) return;
    detailsLoading = true;
    const current = epoch;
    try {
      const result = await dataDetails(token, renamed ? renames.length : conflicts.length, renamed);
      if (disposed || current !== epoch) return;
      conflicts = [...conflicts, ...result.conflicts];
      renames = [...renames, ...result.renames];
    } catch (e) {
      if (!disposed && current === epoch) error = dataError(e);
    } finally {
      detailsLoading = false;
    }
  }
  async function commit() {
    if (!preview || busy || pending || !additions) return;
    busy = true;
    phase = 'importing';
    error = '';
    notice = '';
    try {
      const result = await dataCommit(token);
      if (result.committed) {
        token = '';
        imported = result.summary;
        page = 'imported';
        if (result.refresh_failed) notice = m.data_refresh_failed();
      } else {
        preview = result.preview;
        notice = m.data_reconfirm();
        conflicts = [];
        renames = [];
        showConflicts = false;
        showRenames = false;
      }
    } catch (e) {
      error = dataError(e);
    } finally {
      busy = false;
    }
  }
  async function exportPackage() {
    if (busy) return;
    busy = true;
    error = '';
    phase = 'saving';
    try {
      const result = await dataExport(profiles, preferences);
      if (result) {
        saved = result;
        page = 'exported';
        last = await dataLastExport();
      }
    } catch (e) {
      error = dataError(e);
    } finally {
      busy = false;
    }
  }
  function value(field: string, text: string) {
    try {
      const v = JSON.parse(text);
      if (field === 'completed') return v ? m.detail_completed() : m.detail_incomplete();
      if (field === 'archived') return v ? m.category_archived() : m.category_restore();
      if (field.endsWith('_utc_secs')) return date(v);
      return v === null ? '—' : String(v);
    } catch {
      return text;
    }
  }
  const field = (key: string) =>
    (
      ({
        name: m.category_name,
        archived: m.category_archived,
        completed: m.csv_status,
        duration_secs: m.csv_planned,
        started_at_utc_secs: m.csv_start,
        ended_at_utc_secs: m.csv_end,
        category_stable_id: m.csv_category_id,
        round_type: m.data_type,
        config: m.data_profiles,
        initial_name: m.category_name,
        time_work_secs: m.timer_slider_focus,
        time_short_break_secs: m.timer_slider_short_break,
        time_long_break_secs: m.timer_slider_long_break,
        long_break_interval: m.timer_slider_rounds,
        short_breaks_enabled: m.round_label_short_break,
        long_breaks_enabled: m.round_label_long_break,
        auto_start_work: m.timer_toggle_auto_start_work,
        auto_start_break: m.timer_toggle_auto_start_break,
      }) as Record<string, () => string>
    )[key]?.() ?? m.data_profiles();
  onMount(() => {
    const stops: (() => void)[] = [];
    const keep = async (p: Promise<() => void>) => {
      try {
        const stop = await p;
        if (disposed) stop();
        else stops.push(stop);
      } catch {
        error = m.data_error();
      }
    };
    void dataLastExport()
      .then((v) => {
        if (!disposed) last = v;
      })
      .catch((e) => {
        if (!disposed) error = dataError(e);
      });
    void getTimerState().then((v) => {
      if (!disposed) activeRound = v.has_started;
    });
    void keep(onTimerState((v) => (activeRound = v.has_started)));
    void keep(
      onDataPhase((v) => {
        if (v.token === token) phase = v.phase;
      })
    );
    void keep(
      getCurrentWindow().onCloseRequested((e) => {
        if (busy) e.preventDefault();
      })
    );
    const escape = (e: KeyboardEvent) => {
      if (
        e.key === 'Escape' &&
        page !== 'home' &&
        !document.querySelector('dialog[open],[popover]:popover-open')
      ) {
        e.preventDefault();
        e.stopImmediatePropagation();
        if (!busy) void back();
      }
    };
    document.addEventListener('keydown', escape, true);
    return () => {
      disposed = true;
      ++epoch;
      stops.forEach((s) => s());
      document.removeEventListener('keydown', escape, true);
      if (token) void dataCancel(token);
    };
  });
</script>

<section class="data-flow data-section">
  {#if page !== 'home'}<button class="text-button back" disabled={busy} onclick={back}
      >{m.data_back()}</button
    >{/if}
  {#if page === 'home'}
    <h2>{m.data_title()}</h2>
    <h3>{m.data_package()}</h3>
    <p class="muted">{m.data_package_hint()}</p>
    <div class="setting-row">
      <span>{m.data_export_title()}</span><button
        id="data-export-entry"
        onclick={() => enter('export', 'data-export-entry')}>{m.data_export_button()}</button
      >
    </div>
    <div class="setting-row">
      <span>{m.data_import_title()}</span><button id="data-import-entry" onclick={choose}
        >{m.data_choose()}</button
      >
    </div>
    <hr />
    <h3>{m.data_reports()}</h3>
    <p class="muted">{m.data_reports_hint()}</p>
    <div class="setting-row">
      <span>{m.data_csv()}</span><button
        id="data-report-entry"
        onclick={() => enter('report', 'data-report-entry')}>{m.data_choose_scope()}</button
      >
    </div>
    <hr />
    <p class="muted">{m.data_last({ time: last ? date(last) : m.data_never() })}</p>
  {:else if page === 'export'}
    <h2 id="data-heading" tabindex="-1">{m.data_export_title()}</h2>
    <p>{m.data_format_label()}</p>
    <p>{m.data_export_range()}</p>
    <div class="checks">
      <label><input type="checkbox" checked disabled />{m.data_history()}</label><label
        ><input type="checkbox" bind:checked={profiles} disabled={busy} />{m.data_profiles()}</label
      ><label
        ><input
          type="checkbox"
          bind:checked={preferences}
          disabled={busy}
        />{m.data_preferences()}</label
      >
    </div>
    <p class="muted">{m.data_export_notes()}</p>
    <p class="muted">{m.data_external_themes()}</p>
    <p class="muted">{m.data_private()}</p>
    <div class="actions">
      <button disabled={busy} onclick={back}>{m.data_cancel()}</button><button
        class="primary"
        disabled={busy}
        onclick={exportPackage}>{m.data_save_location()}</button
      >
    </div>
  {:else if page === 'exported' && saved}
    <h2 id="data-heading" tabindex="-1">{m.data_export_title()}</h2>
    <p role="status">
      {m.data_saved({
        filename: saved.filename,
        rows: saved.rows,
        categories: saved.categories,
        profiles: saved.profiles,
      })}
    </p>
    {#if saved.warning}<p>{m.data_last_failed()}</p>{/if}
    <div class="actions"><button onclick={back}>{m.data_done()}</button></div>
  {:else if page === 'import'}
    <h2 id="data-heading" tabindex="-1">{m.data_ready()}</h2>
    {#if preview}
      <p>{preview.filename}</p>
      <p class="muted">
        {m.data_exported_at({ time: date(preview.exported_at) })}<br />{m.data_dates({
          start: date(preview.start),
          end: date(preview.end),
        })}<br />{m.data_source_version({ version: preview.app_version })}<br
        />{m.data_source_timezone({ zone: preview.source_timezone ?? m.data_unknown() })}
      </p>
      <div class="checks">
        <label
          ><input
            type="checkbox"
            checked={preview.options.history}
            disabled={pending || busy}
            onchange={(e) => options('history', e.currentTarget.checked)}
          />{m.data_history()}</label
        >
        {#if preview.has_profiles}<label
            ><input
              type="checkbox"
              checked={preview.options.profiles}
              disabled={pending || busy}
              onchange={(e) => options('profiles', e.currentTarget.checked)}
            />{m.data_profiles()}</label
          >{/if}
        {#if preview.has_preferences}<label
            ><input
              type="checkbox"
              checked={preview.options.preferences}
              disabled={pending || busy}
              onchange={(e) => options('preferences', e.currentTarget.checked)}
            />{m.data_preferences()}</label
          >{/if}
      </div>
      <p class="muted">{m.data_timezone_hint()}</p>
      {#if !pending}
        <table>
          <thead
            ><tr
              ><th>{m.data_type()}</th><th>{m.data_new()}</th><th>{m.data_existing()}</th><th
                >{m.data_conflict_count()}</th
              ></tr
            ></thead
          ><tbody>
            {#each [['sessions', m.data_sessions()], ['categories', m.data_categories()], ['profiles', m.data_profiles()]] as [key, label]}
              {#if key === 'profiles' ? preview.options.profiles : preview.options.history}{@const count =
                  preview.summary[key as 'sessions' | 'categories' | 'profiles']}<tr
                  ><td>{label}</td><td>{count.added}</td><td>{count.existing}</td><td
                    >{count.conflicts}</td
                  ></tr
                >{/if}
            {/each}
          </tbody>
        </table>
        {#if preview.duplicates}<p class="muted">
            {m.data_duplicates({ count: preview.duplicates })}
          </p>{/if}
        {#if preview.conflicts}
          <button
            class="text-button"
            aria-expanded={showConflicts}
            disabled={busy}
            onclick={() => {
              showConflicts = !showConflicts;
              if (showConflicts && !conflicts.length) void detail(false);
            }}>{m.data_conflicts({ count: preview.conflicts })}</button
          >
          {#if showConflicts}{#each conflicts as c}<div class="details-item">
                <strong
                  >{kind(c.kind)} · {c.kind === 'session' ? date(Number(c.label)) : c.label}</strong
                >{#each c.differences as d}<p>
                    {field(d.field)} · {m.data_local_file({
                      local: value(d.field, d.local),
                      file: value(d.field, d.file),
                    })}
                  </p>{/each}
                <p>{m.data_keep_action()}</p>
              </div>{/each}{#if conflicts.length < preview.conflicts}<button
                disabled={detailsLoading || busy}
                onclick={() => detail(false)}>{m.data_more()}</button
              >{/if}{/if}
          {#if preview.summary.categories.conflicts}<p class="muted">
              {m.data_category_conflict_hint()}
            </p>{/if}
        {/if}
        {#if preview.renames}
          <button
            class="text-button"
            aria-expanded={showRenames}
            disabled={busy}
            onclick={() => {
              showRenames = !showRenames;
              if (showRenames && !renames.length) void detail(true);
            }}>{m.data_renames({ count: preview.renames })}</button
          >
          {#if showRenames}<p class="muted">{m.data_rename_hint()}</p>
            {#each renames as r}<p>
                {r.from} → {r.to}
              </p>{/each}{#if renames.length < preview.renames}<button
                disabled={detailsLoading || busy}
                onclick={() => detail(true)}>{m.data_more()}</button
              >{/if}{/if}
        {/if}
        <p class="muted">{m.data_keep_local()}</p>
        {#if !additions}<p>
            {preview.conflicts ? m.data_only_conflicts() : m.data_no_changes()}
          </p>{/if}
        {#if activeRound && additions}<p>{m.data_active_round()}</p>{/if}
      {/if}
    {/if}
    <div class="actions">
      <button disabled={busy} onclick={back}>{m.data_cancel()}</button
      >{#if preview && !pending}{#if additions}<button
            class="primary"
            disabled={busy || activeRound}
            onclick={commit}>{m.data_merge()}</button
          >{:else}<button onclick={back}>{m.data_done()}</button>{/if}{:else if !pending}<button
          onclick={choose}>{m.data_choose()}</button
        >{/if}
    </div>
  {:else if page === 'imported' && imported}
    <h2 id="data-heading" tabindex="-1">{m.data_import_title()}</h2>
    <p role="status">
      {m.data_imported({
        added: imported.sessions.added,
        existing: imported.sessions.existing,
        conflicts:
          imported.sessions.conflicts + imported.categories.conflicts + imported.profiles.conflicts,
      })}
    </p>
    <p>
      {m.data_imported_extras({
        categories: imported.categories.added,
        profiles: imported.profiles.added,
      })}
    </p>
    <div class="actions">
      <button onclick={back}>{m.data_done()}</button><button
        onclick={() => dataViewStats().catch((e) => (error = dataError(e)))}
        >{m.data_view_stats()}</button
      >
    </div>
  {:else if page === 'report'}<ReportForm onclose={back} onbusy={(value) => (busy = value)} />{/if}
  {#if (pending || busy) && page !== 'report'}<p role="status">
      {phase === 'importing'
        ? m.data_importing()
        : phase === 'saving'
          ? m.data_saving()
          : phase === 'validating'
            ? m.data_validating()
            : m.data_reading()}
    </p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}{#if error}<p role="alert">{error}</p>{/if}
</section>

<style>
  .data-section {
    padding: 24px;
  }
  .setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    min-height: 56px;
  }
  .setting-row button {
    flex: none;
  }
  .back {
    margin: 0 0 12px;
  }
  h2:focus-visible {
    outline: 1px solid var(--color-focus-round);
    outline-offset: 2px;
  }
  @media (max-width: 600px) {
    .data-section {
      padding: 16px;
    }
    .setting-row {
      align-items: flex-start;
      flex-direction: column;
      gap: 6px;
      padding: 8px 0;
    }
  }
</style>
