<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { plans, actOnPlan, connectPlans, planLoadError } from '$lib/plans/state';
  import { planName, planError } from '$lib/plans/format';
  import SmallMenu from './SmallMenu.svelte';
  import * as m from '$paraglide/messages.js';
  let mode = $state<'save_as' | 'rename' | 'template' | 'delete' | null>(null);
  let name = $state('');
  let target = $state('');
  let long = false;
  let error = $state('');
  let busy = $state(false);
  let input = $state<HTMLInputElement>();
  let selector: HTMLSelectElement;
  let cancelButton = $state<HTMLButtonElement>();
  let more: SmallMenu;
  const selected = $derived($plans?.book.plans.find((p) => p.id === $plans?.book.selected_id));
  const protectedPlan = (id: string) =>
    $plans?.book.plans.length === 1 ||
    id === $plans?.active_id ||
    id === $plans?.pending_id ||
    id === $plans?.book.selected_id;
  onMount(() => {
    const connection = connectPlans();
    return connection.dispose;
  });
  async function perform(fn: () => Promise<unknown>, finish = false) {
    if (busy) return;
    busy = true;
    error = '';
    let succeeded = false;
    try {
      await fn();
      succeeded = true;
      if (finish) mode = null;
    } catch (e) {
      error = planError(e);
    } finally {
      busy = false;
    }
    if (finish && succeeded) {
      await tick();
      selector.focus();
    }
  }
  async function edit(next: typeof mode, templateLong = false) {
    more?.close();
    error = '';
    mode = next;
    long = templateLong;
    target = selected?.id ?? '';
    name =
      next === 'rename' && selected
        ? planName(selected)
        : next === 'template'
          ? long
            ? m.plan_template_long()
            : m.plan_template_standard()
          : '';
    if (next === 'delete') target = $plans?.book.plans.find((p) => !protectedPlan(p.id))?.id ?? '';
    await tick();
    if (next === 'delete') cancelButton?.focus();
    else {
      input?.focus();
      input?.select();
    }
  }
  async function cancel() {
    mode = null;
    error = '';
    await tick();
    selector.focus();
  }
  function editorKeys(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape') {
      event.preventDefault();
      void cancel();
    }
  }
  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (mode === 'delete') return perform(() => actOnPlan({ kind: 'delete', id: target }), true);
    if (mode === 'rename')
      return perform(() => actOnPlan({ kind: 'rename', id: target, name }), true);
    if (mode === 'save_as') return perform(() => actOnPlan({ kind: 'save_as', name }), true);
    if (mode === 'template')
      return perform(() => actOnPlan({ kind: 'template', name, long }), true);
  }
</script>

<section id="timer-plan-region" tabindex="-1" class="plans" aria-label={m.plan_title()}>
  <div class="heading">
    <span>{m.plan_title()}</span>{#if $plans?.modified}<span class="modified"
        >{m.plan_modified()}</span
      >{/if}
  </div>
  <div class="toolbar">
    <select
      id="timer-plan-select"
      bind:this={selector}
      aria-label={m.plan_title()}
      disabled={busy || !$plans}
      value={$plans?.book.selected_id}
      onchange={(event) =>
        perform(() => actOnPlan({ kind: 'select', id: event.currentTarget.value }))}
      onkeydown={(e) => e.stopPropagation()}
    >
      {#each $plans?.book.plans ?? [] as plan (plan.id)}<option value={plan.id}
          >{planName(plan)}</option
        >{/each}
    </select>
    <button
      disabled={busy || !$plans?.modified}
      onclick={() => perform(() => actOnPlan({ kind: 'save' }))}>{m.plan_save_changes()}</button
    >
    <button disabled={busy || !$plans} onclick={() => edit('save_as')}>{m.plan_save_as()}</button>
    <SmallMenu bind:this={more} label={m.plan_more()} icon>
      <button role="menuitem" disabled={busy || !$plans} onclick={() => edit('rename')}
        >{m.plan_rename()}</button
      >
      <button role="menuitem" disabled={busy || !$plans} onclick={() => edit('delete')}
        >{m.plan_delete()}</button
      >
      <hr />
      <div class="menu-note">{m.plan_from_template()}</div>
      <button role="menuitem" disabled={busy} onclick={() => edit('template', false)}
        >{m.plan_template_standard()}</button
      >
      <button role="menuitem" disabled={busy} onclick={() => edit('template', true)}
        >{m.plan_template_long()}</button
      >
    </SmallMenu>
  </div>
  {#if $plans?.pending_id}<p class="hint">
      {m.plan_pending({
        name: planName($plans.book.plans.find((p) => p.id === $plans?.pending_id)!),
      })}
    </p>{/if}
  {#if mode}
    <form class="editor" onsubmit={submit}>
      {#if mode === 'delete'}
        <label for="plan-delete">{m.plan_delete_choose()}</label>
        <select onkeydown={editorKeys} id="plan-delete" bind:value={target} disabled={busy}>
          <option value="" disabled>{m.plan_delete_choose()}</option>
          {#each $plans?.book.plans ?? [] as plan (plan.id)}<option
              value={plan.id}
              disabled={protectedPlan(plan.id)}
              >{planName(plan)}{protectedPlan(plan.id)
                ? ` · ${m.plan_active_or_pending()}`
                : ''}</option
            >{/each}
        </select>
        <p class="hint">
          {target
            ? m.plan_delete_confirm({
                name: planName($plans!.book.plans.find((p) => p.id === target)!),
              })
            : $plans?.book.plans.length === 1
              ? m.plan_last()
              : m.plan_in_use()}
        </p>
      {:else}
        <label for="plan-name"
          >{mode === 'rename'
            ? m.plan_rename()
            : mode === 'template'
              ? m.plan_from_template()
              : m.plan_save_as()}</label
        >
        <input
          onkeydown={editorKeys}
          id="plan-name"
          bind:this={input}
          bind:value={name}
          autocomplete="off"
          disabled={busy}
          aria-describedby="plan-name-hint"
        />
        <p id="plan-name-hint" class="hint">{m.plan_name_hint()}</p>
      {/if}
      <div class="actions">
        <button
          onkeydown={editorKeys}
          bind:this={cancelButton}
          type="button"
          disabled={busy}
          onclick={cancel}>{m.plan_cancel()}</button
        ><button
          onkeydown={editorKeys}
          type="submit"
          disabled={busy || (mode === 'delete' && (!target || protectedPlan(target)))}
          >{mode === 'delete' ? m.plan_confirm_delete() : m.plan_save()}</button
        >
      </div>
    </form>
  {/if}
  {#if error || $planLoadError}<p class="hint" role="alert">{error || m.plan_error()}</p>{/if}
</section>

<style>
  .plans:focus-visible {
    outline: 1px solid var(--color-foreground);
    outline-offset: -2px;
  }
  .plans {
    padding: 12px 20px;
    border-bottom: 1px solid var(--color-separator);
    font-size: 12px;
  }
  .heading {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }
  .modified,
  .hint {
    color: var(--color-foreground-darker);
    font-size: 11px;
    line-height: 1.5;
  }
  .hint {
    margin-top: 6px;
    overflow-wrap: anywhere;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-wrap: wrap;
  }
  select,
  input {
    min-width: 0;
    min-height: 30px;
    border: 1px solid var(--color-separator);
    border-radius: 4px;
    color: var(--color-foreground);
    background: var(--color-background-light);
    font: inherit;
    padding: 4px 6px;
  }
  .toolbar select {
    flex: 1;
    width: 120px;
  }
  button {
    background: transparent;
    border: 0;
    border-radius: 4px;
    color: var(--color-foreground);
    font: inherit;
    min-height: 30px;
    padding: 4px 6px;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--color-hover);
  }
  button:disabled,
  select:disabled {
    opacity: 0.55;
    cursor: default;
  }
  button:focus-visible,
  input:focus-visible,
  select:focus-visible {
    outline: 1px solid var(--color-foreground);
    outline-offset: 1px;
  }
  .editor {
    border-top: 1px solid var(--color-separator);
    margin-top: 10px;
    padding-top: 10px;
  }
  .editor label {
    display: block;
    margin-bottom: 6px;
  }
  .editor input,
  .editor select {
    width: 100%;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 6px;
    margin-top: 6px;
  }
</style>
