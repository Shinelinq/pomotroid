<script lang="ts">
  import { onMount } from 'svelte';
  import SmallMenu from '$lib/components/plans/SmallMenu.svelte';
  import {
    categories,
    categoryLoadError,
    connectCategories,
    actOnCategory,
  } from '$lib/categories/state';
  import { categoryName, categoryError, timerCategoryLabel } from '$lib/categories/format';
  import { timerState } from '$lib/stores/timer';
  import { manageCategories } from '$lib/ipc';
  import * as m from '$paraglide/messages.js';
  let menu: SmallMenu;
  let error = $state('');
  let busy = $state(false);
  const items = $derived($categories?.data.items ?? []);
  const label = $derived(timerCategoryLabel(items, $timerState));
  onMount(() => {
    const connection = connectCategories(false);
    return connection.dispose;
  });
  async function choose(id: number | null) {
    if (busy) return;
    busy = true;
    error = '';
    try {
      await actOnCategory({ kind: 'select', id });
      menu.close();
    } catch (e) {
      error = categoryError(e);
    } finally {
      busy = false;
    }
  }
</script>

<SmallMenu bind:this={menu} {label} title={label}>
  <button
    role="menuitemradio"
    aria-checked={$timerState.next_category_id == null}
    disabled={busy}
    onclick={() => choose(null)}>{m.category_uncategorized()}</button
  >
  {#each items.filter((item) => !item.archived) as item (item.id)}
    <button
      role="menuitemradio"
      aria-checked={$timerState.next_category_id === item.id}
      disabled={busy}
      onclick={() => choose(item.id)}>{item.name}</button
    >
  {/each}
  {#if $timerState.category_pending}<p class="menu-note">
      {m.category_pending({ name: categoryName(items, $timerState.next_category_id) })}
    </p>{/if}
  {#if $timerState.category_notice_id != null}<p class="menu-note">
      {m.category_archive_fallback({
        name: categoryName(items, $timerState.category_notice_id, false),
      })}
    </p>{/if}
  <hr />
  <button
    role="menuitem"
    onclick={async () => {
      try {
        await manageCategories();
        menu.close();
      } catch (e) {
        error = categoryError(e);
      }
    }}>{m.category_manage()}</button
  >
  {#if error || $categoryLoadError}<p class="menu-note" role="alert">
      {error || m.category_error()}
    </p>{/if}
</SmallMenu>
