<script lang="ts">
  import { plans, actOnPlan, planLoadError } from '$lib/plans/state';
  import { planName, planSummary, planError } from '$lib/plans/format';
  import { manageTimerPlans } from '$lib/ipc';
  import SmallMenu from './SmallMenu.svelte';
  import CategoryPicker from '$lib/components/categories/CategoryPicker.svelte';
  import * as m from '$paraglide/messages.js';
  let menu: SmallMenu;
  let error = $state('');
  let busy = $state(false);
  const current = $derived($plans?.book.plans.find((p) => p.id === $plans?.active_id));
  async function choose(id: string) {
    busy = true;
    error = '';
    try {
      await actOnPlan({ kind: 'select', id });
      menu.close();
    } catch (e) {
      error = planError(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="context-row">
  <div class="plan-entry">
    <SmallMenu bind:this={menu} label={current ? planName(current) : m.plan_title()}>
      {#each $plans?.book.plans ?? [] as plan (plan.id)}
        <button role="menuitem" disabled={busy} onclick={() => choose(plan.id)}>
          <span class="name">{planName(plan)}</span>
          {#if plan.id === $plans?.active_id}<span class="tag">{m.plan_active()}</span>{/if}
          {#if plan.id === $plans?.pending_id}<span class="tag">{m.plan_next()}</span>{/if}
          <small>{planSummary(plan.config)}</small>
        </button>
      {/each}
      <hr />
      <button
        role="menuitem"
        onclick={async () => {
          try {
            await manageTimerPlans();
            menu.close();
          } catch (e) {
            error = planError(e);
          }
        }}>{m.plan_manage()}</button
      >
      {#if error || $planLoadError}<p class="menu-note" role="alert">
          {error || m.plan_error()}
        </p>{/if}
    </SmallMenu>
  </div>
  <div class="category-entry"><CategoryPicker /></div>
</div>

<style>
  .context-row {
    height: 28px;
    flex: none;
    display: flex;
    align-items: center;
    padding: 0 12px;
    gap: 4px;
    justify-content: space-between;
  }
  .plan-entry,
  .category-entry {
    flex: 1;
    min-width: 0;
    max-width: 50%;
  }
  .category-entry {
    display: flex;
    justify-content: flex-end;
  }
  .name {
    overflow-wrap: anywhere;
  }
  .tag {
    margin-left: 8px;
    font-size: 11px;
    color: var(--color-foreground-darker);
    white-space: nowrap;
  }
</style>
