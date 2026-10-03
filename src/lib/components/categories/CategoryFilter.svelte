<script lang="ts">
  import { onMount } from 'svelte';
  import type { CategoryFilter } from '$lib/types';
  import SmallMenu from '$lib/components/plans/SmallMenu.svelte';
  import { categories, categoryLoadError, connectCategories } from '$lib/categories/state';
  import { filterName, filterKey } from '$lib/categories/format';
  import * as m from '$paraglide/messages.js';
  let { value, onchange }: { value: CategoryFilter; onchange: (value: CategoryFilter) => void } =
    $props();
  let menu: SmallMenu;
  const items = $derived($categories?.data.items ?? []);
  const archived = $derived(items.filter((c) => c.archived));
  onMount(() => {
    const connection = connectCategories(false);
    return connection.dispose;
  });
  function choose(next: CategoryFilter) {
    onchange(next);
    menu.close();
  }
</script>

<SmallMenu
  bind:this={menu}
  label={filterName(items, value)}
  title={`${m.category_filter()}: ${filterName(items, value)}`}
>
  <button
    role="menuitemradio"
    aria-checked={value.kind === 'all'}
    onclick={() => choose({ kind: 'all' })}>{m.category_all()}</button
  >
  <button
    role="menuitemradio"
    aria-checked={value.kind === 'uncategorized'}
    onclick={() => choose({ kind: 'uncategorized' })}>{m.category_uncategorized()}</button
  >
  {#each items.filter((c) => !c.archived) as item (item.id)}
    <button
      role="menuitemradio"
      aria-checked={filterKey(value) === `category:${item.id}`}
      onclick={() => choose({ kind: 'category', category_id: item.id })}>{item.name}</button
    >
  {/each}
  {#if archived.length}<hr />
    <div class="menu-note">{m.category_archived()}</div>
    {#each archived as item (item.id)}
      <button
        role="menuitemradio"
        aria-checked={filterKey(value) === `category:${item.id}`}
        onclick={() => choose({ kind: 'category', category_id: item.id })}>{item.name}</button
      >
    {/each}
  {/if}
  {#if $categoryLoadError}<p class="menu-note" role="alert">{m.category_error()}</p>{/if}
</SmallMenu>
