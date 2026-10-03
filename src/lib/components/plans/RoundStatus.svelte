<script lang="ts">
  import { tick } from 'svelte';
  import { categories, actOnCategory } from '$lib/categories/state';
  import { categoryName, categoryError } from '$lib/categories/format';
  import { plans, actOnPlan } from '$lib/plans/state';
  import { timerState } from '$lib/stores/timer';
  import { planName, planError } from '$lib/plans/format';
  import { eta } from '$lib/plans/eta';
  import { getLocale } from '$paraglide/runtime.js';
  import SmallMenu from './SmallMenu.svelte';
  import * as m from '$paraglide/messages.js';
  let menu: SmallMenu;
  let arrangements = $state<SmallMenu>();
  let error = $state('');
  let confirming = $state(false);
  let busy = $state(false);
  let confirm: HTMLDialogElement;
  let cancelButton: HTMLButtonElement;
  let confirmation = { round_id: 0, pending_revision: 0 };
  const end = $derived(eta($timerState, getLocale()));
  const status = $derived(
    end.kind === 'waiting'
      ? m.round_waiting()
      : end.kind === 'paused'
        ? m.round_paused({ minutes: end.minutes, seconds: end.seconds })
        : end.days === 0
          ? m.round_eta({ time: end.time })
          : end.days === 1
            ? m.round_eta_tomorrow({ time: end.time })
            : m.round_eta_date({ date: end.date, time: end.time })
  );
  const pending = $derived($plans?.book.plans.find((p) => p.id === $plans?.pending_id));
  const categoryItems = $derived($categories?.data.items ?? []);
  const categoryPending = $derived($timerState.category_pending);
  const categoryNotice = $derived($timerState.category_notice_id != null);
  const categoryArranged = $derived(categoryPending || categoryNotice);
  const canCancelCategory = $derived(
    !$timerState.category_id ||
      categoryItems.some((c) => c.id === $timerState.category_id && !c.archived)
  );
  const categoryLine = $derived(
    categoryPending
      ? m.category_pending({ name: categoryName(categoryItems, $timerState.next_category_id) })
      : m.category_archive_fallback({
          name: categoryName(categoryItems, $timerState.category_notice_id, false),
        })
  );
  const count = $derived(
    Number(!!pending) + Number($timerState.stop_after_round) + Number(categoryArranged)
  );
  const singleLine = $derived(
    pending
      ? m.plan_pending({ name: planName(pending) })
      : categoryArranged
        ? categoryLine
        : m.round_stop_arranged()
  );
  function returnCategoryFocus() {
    if (!arrangements?.closeIfOpen() && !menu?.closeIfOpen()) menu?.close();
  }
  const cancelCategory = () =>
    action(async () => {
      await actOnCategory({ kind: 'cancel_pending', round_id: $timerState.round_id });
      returnCategoryFocus();
    });
  const dismissCategory = () =>
    action(async () => {
      await actOnCategory({ kind: 'dismiss_notice' });
      returnCategoryFocus();
    });
  function cancelSingle() {
    if (pending) return action(() => actOnPlan({ kind: 'cancel_pending' }));
    if (categoryArranged) return categoryPending ? cancelCategory() : dismissCategory();
    return stop(false);
  }
  async function stop(enabled: boolean) {
    await action(() => actOnPlan({ kind: 'stop_after', enabled, round_id: $timerState.round_id }));
  }
  async function action(fn: () => Promise<unknown>) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await fn();
    } catch (e) {
      error = String(e).startsWith('category_') ? categoryError(e) : planError(e);
    } finally {
      busy = false;
    }
  }
  async function askApply() {
    confirmation = { round_id: $timerState.round_id, pending_revision: $plans!.pending_revision };
    arrangements?.close();
    menu?.close();
    confirming = true;
    confirm.showModal();
    await tick();
    cancelButton.focus();
  }
  async function applyNow() {
    await action(async () => {
      await actOnPlan({ kind: 'apply_now', ...confirmation });
      confirm.close();
    });
  }
</script>

{#snippet categoryControls()}
  {#if categoryArranged}<hr />
    <div class="menu-note">{categoryLine}</div>
    {#if categoryPending}
      <button role="menuitem" disabled={busy || !canCancelCategory} onclick={cancelCategory}
        >{m.category_cancel_pending()}</button
      >
      {#if !canCancelCategory}<p class="menu-note">{m.category_archived_error()}</p>{/if}
    {/if}
    {#if categoryNotice}
      {#if categoryPending}<p class="menu-note">
          {m.category_archive_fallback({
            name: categoryName(categoryItems, $timerState.category_notice_id, false),
          })}
        </p>{/if}
      <button role="menuitem" disabled={busy} onclick={dismissCategory}
        >{m.category_dismiss()}</button
      >
    {/if}
  {/if}
{/snippet}
<div class="round-status">
  <div class="status-line">
    <span>{status}</span>
    <SmallMenu bind:this={menu} label={m.round_more()} icon>
      <button
        role="menuitemcheckbox"
        aria-checked={$timerState.stop_after_round}
        disabled={busy || !$timerState.has_started}
        onclick={() => stop(!$timerState.stop_after_round)}
      >
        {$timerState.stop_after_round ? '✓ ' : ''}{m.round_stop()}
        {#if !$timerState.has_started}<small>{m.round_stop_disabled()}</small>{/if}
      </button>
      {#if pending}<hr />
        <div class="menu-note">{m.plan_pending({ name: planName(pending) })}</div>
        <button
          role="menuitem"
          disabled={busy}
          onclick={() => action(() => actOnPlan({ kind: 'cancel_pending' }))}
          >{m.plan_cancel_pending()}</button
        >
        <button role="menuitem" disabled={busy} onclick={askApply}>{m.plan_apply_now()}</button>
      {/if}
      {@render categoryControls()}
    </SmallMenu>
  </div>
  {#if count > 0}
    <div class="arranged">
      {#if count === 1}
        <span title={singleLine}>{singleLine}</span><span>·</span>
        <button disabled={busy || (categoryPending && !canCancelCategory)} onclick={cancelSingle}>
          {categoryArranged && !categoryPending ? m.category_dismiss() : m.plan_cancel()}
        </button>
      {:else}<span>{m.round_arranged_count({ count })}</span><span>·</span>{/if}
      <SmallMenu
        bind:this={arrangements}
        label={count > 1 ? m.round_view() : m.round_more()}
        icon={count === 1}
      >
        {#if $timerState.stop_after_round}<div class="menu-note">{m.round_stop_arranged()}</div>
          <button role="menuitem" disabled={busy} onclick={() => stop(false)}
            >{m.round_cancel_stop()}</button
          >{/if}
        {#if pending}<hr />
          <div class="menu-note">{m.plan_pending({ name: planName(pending) })}</div>
          <button
            role="menuitem"
            disabled={busy}
            onclick={() => action(() => actOnPlan({ kind: 'cancel_pending' }))}
            >{m.plan_cancel_pending()}</button
          >
          <button role="menuitem" disabled={busy} onclick={askApply}>{m.plan_apply_now()}</button>
        {/if}
        {@render categoryControls()}
      </SmallMenu>
    </div>
  {/if}
  {#if error && !confirming}<p class="error" role="alert">{error}</p>{/if}
</div>
<dialog
  bind:this={confirm}
  onkeydown={(e) => e.stopPropagation()}
  onclose={() => {
    confirming = false;
    (arrangements ?? menu)?.close();
  }}
>
  <p>{m.plan_apply_confirm()}</p>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  <div class="dialog-actions">
    <button bind:this={cancelButton} disabled={busy} onclick={() => confirm.close()}
      >{m.plan_cancel()}</button
    ><button disabled={busy} onclick={applyNow}>{m.plan_confirm_apply()}</button>
  </div>
</dialog>

<style>
  .round-status {
    width: min(100%, 310px);
    font-size: 11px;
    color: var(--color-foreground-darker);
  }
  .status-line,
  .arranged {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 28px;
    gap: 4px;
  }
  .arranged > span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    background: transparent;
    color: inherit;
    border: 0;
    border-radius: 4px;
    padding: 4px 6px;
    min-height: 28px;
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    background: var(--color-hover);
  }
  button:focus-visible {
    outline: 1px solid var(--color-foreground);
  }
  button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .error {
    line-height: 1.5;
    padding: 4px;
  }
  dialog {
    margin: auto;
    padding: 16px;
    width: 300px;
    max-width: calc(100vw - 24px);
    border: 1px solid var(--color-separator);
    border-radius: 4px;
    background: var(--color-background-light);
    color: var(--color-foreground);
    font-size: 12px;
    line-height: 1.6;
  }
  dialog::backdrop {
    background: color-mix(in oklch, var(--color-background) 75%, transparent);
  }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>
