<script lang="ts">
  import { onMount, tick } from 'svelte';
  import {
    categories,
    categoryLoadError,
    connectCategories,
    actOnCategory,
  } from '$lib/categories/state';
  import { categoryError, categoryName } from '$lib/categories/format';
  import { timerState } from '$lib/stores/timer';
  import type { Category } from '$lib/types';
  import * as m from '$paraglide/messages.js';
  let editing = $state<number | 'new' | null>(null);
  let name = $state('');
  let error = $state('');
  let busy = $state(false);
  let input = $state<HTMLInputElement>();
  let newButton: HTMLButtonElement;
  let origin: HTMLElement | null = null;
  const items = $derived($categories?.data.items ?? []);
  const active = $derived(items.filter((item) => !item.archived));
  const archived = $derived(items.filter((item) => item.archived));
  onMount(() => {
    const connection = connectCategories();
    return connection.dispose;
  });
  async function edit(item?: Category) {
    origin = document.activeElement as HTMLElement;
    editing = item?.id ?? 'new';
    name = item?.name ?? '';
    error = '';
    await tick();
    input?.focus();
    input?.select();
  }
  async function cancel() {
    editing = null;
    error = '';
    await tick();
    if (origin?.isConnected) origin.focus();
    else newButton.focus();
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (busy || editing === null) return;
    busy = true;
    error = '';
    try {
      await actOnCategory(
        editing === 'new' ? { kind: 'create', name } : { kind: 'rename', id: editing, name }
      );
      editing = null;
    } catch (e) {
      error = categoryError(e);
    } finally {
      busy = false;
    }
    if (editing === null) {
      await tick();
      if (origin?.isConnected) origin.focus();
      else newButton.focus();
    }
  }
  async function archive(item: Category, button: HTMLButtonElement) {
    busy = true;
    error = '';
    editing = null;
    try {
      await actOnCategory({ kind: item.archived ? 'restore' : 'archive', id: item.id });
    } catch (e) {
      error = categoryError(e);
    } finally {
      busy = false;
    }
    await tick();
    if (button.isConnected) button.focus();
    else newButton.focus();
  }
  function keys(event: KeyboardEvent) {
    event.stopPropagation();
    if (event.key === 'Escape' && !busy) {
      event.preventDefault();
      void cancel();
    }
  }
</script>

{#snippet editor()}
  <form onsubmit={save} class="editor">
    <label for="category-name">{m.category_name()}</label>
    <input
      bind:this={input}
      id="category-name"
      bind:value={name}
      disabled={busy}
      placeholder={m.category_example()}
      autocomplete="off"
      onkeydown={keys}
      aria-describedby="category-name-hint"
    />
    <div class="edit-actions">
      <button type="button" disabled={busy} onkeydown={keys} onclick={cancel}
        >{m.plan_cancel()}</button
      ><button type="submit" disabled={busy} onkeydown={keys}>{m.plan_save()}</button>
    </div>
    <p id="category-name-hint">{m.plan_name_hint()}</p>
    {#if error}<p role="alert">{error}</p>{/if}
  </form>
{/snippet}
{#snippet row(item: Category)}
  {#if editing === item.id}{@render editor()}
  {:else}
    <div class="category-row">
      <span title={item.name}>{item.name}</span>
      <div class="row-actions">
        <button
          disabled={busy}
          onclick={() => edit(item)}
          aria-label={`${m.category_rename()} ${item.name}`}>{m.category_rename()}</button
        >
        <button
          disabled={busy}
          onclick={(event) => archive(item, event.currentTarget)}
          aria-label={`${item.archived ? m.category_restore() : m.category_archive()} ${item.name}`}
          >{item.archived ? m.category_restore() : m.category_archive()}</button
        >
      </div>
    </div>
  {/if}
{/snippet}

<section id="category-region" tabindex="-1" aria-label={m.category_title()}>
  <div class="heading">
    <h2>{m.category_title()}</h2>
    <button bind:this={newButton} disabled={busy} onclick={() => edit()}
      >{m.category_create()}</button
    >
  </div>
  {#if editing === 'new'}{@render editor()}{/if}
  {#if $categoryLoadError}<p class="notice" role="alert">{m.category_error()}</p>{/if}
  {#if error && editing === null}<p class="notice" role="alert">{error}</p>{/if}
  {#if $timerState.category_notice_id != null}<p class="notice" role="status">
      {m.category_archive_fallback({
        name: categoryName(items, $timerState.category_notice_id, false),
      })}
    </p>{/if}
  {#each active as item (item.id)}{@render row(item)}{/each}
  {#if $categories && !$categoryLoadError && !active.length && editing !== 'new'}<p class="notice">
      {items.length ? m.category_no_active() : m.category_empty()}
    </p>{/if}
  <details>
    <summary>{m.category_archived_count({ count: archived.length })}</summary>
    {#each archived as item (item.id)}{@render row(item)}{/each}
  </details>
</section>

<style>
  section {
    padding: 0 20px 16px;
    font-size: 12px;
  }
  .heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 48px;
    border-bottom: 1px solid var(--color-separator);
    gap: 8px;
  }
  h2 {
    font-size: 13px;
    font-weight: 600;
  }
  .category-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 46px;
    gap: 8px;
    border-bottom: 1px solid var(--color-separator);
  }
  .category-row > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row-actions,
  .edit-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
  button {
    min-height: 28px;
    padding: 4px 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    font: inherit;
    cursor: pointer;
  }
  button:hover:not(:disabled) {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  button:focus-visible,
  input:focus-visible,
  summary:focus-visible,
  section:focus-visible {
    outline: 1px solid var(--color-foreground);
    outline-offset: -1px;
  }
  .editor {
    padding: 10px 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 6px;
    border-bottom: 1px solid var(--color-separator);
  }
  .editor label,
  .editor p {
    grid-column: 1 / -1;
  }
  .editor p,
  .notice {
    color: var(--color-foreground-darker);
    font-size: 11px;
    line-height: 1.6;
    overflow-wrap: anywhere;
  }
  input {
    min-width: 0;
    height: 30px;
    padding: 4px 6px;
    border: 1px solid var(--color-separator);
    border-radius: 4px;
    color: var(--color-foreground);
    background: var(--color-background-light);
    font: inherit;
  }
  .notice {
    padding: 10px 0;
  }
  details {
    margin-top: 14px;
  }
  summary {
    cursor: pointer;
    min-height: 28px;
    padding: 5px 0;
    color: var(--color-foreground-darker);
  }
</style>
