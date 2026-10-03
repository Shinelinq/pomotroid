<script lang="ts">
  import * as m from '$paraglide/messages.js';
  import StatsTooltip from './StatsTooltip.svelte';
  import { createChartInteraction } from './interaction.svelte';
  let { label, text }: { label: string; text: string } = $props();
  let button = $state<HTMLButtonElement>();
  const interaction = createChartInteraction();
  const id = `stats-help-${Math.random().toString(36).slice(2)}`;
  const visible = $derived(!!interaction.preview || !!interaction.pinned);
</script>

<svelte:window
  onkeydown={interaction.escape}
  onpointerdown={(event) => {
    const target = event.target as Element;
    if (target !== button && !button?.contains(target) && !target.closest(`#${id}`))
      interaction.clear();
  }}
/>
<button
  bind:this={button}
  class="help"
  aria-label={m.stats_help_for({ label })}
  aria-expanded={visible}
  aria-describedby={visible ? id : undefined}
  onpointerenter={() => interaction.show('help', button!)}
  onpointerleave={interaction.leave}
  onfocus={() => interaction.show('help', button!, true)}
  onblur={interaction.leave}
  onclick={() => {
    if (interaction.pinned) interaction.clear();
    else {
      interaction.toggle('help');
      interaction.show('help', button!, true);
    }
  }}
>
  <svg width="12" height="12" viewBox="0 0 13 13" fill="none" aria-hidden="true"
    ><circle cx="6.5" cy="6.5" r="5.5" stroke="currentColor" stroke-width="1.3" /><path
      d="M6.5 6v3.5"
      stroke="currentColor"
      stroke-width="1.3"
    /><circle cx="6.5" cy="3.8" r=".8" fill="currentColor" /></svg
  >
</button>
{#if visible && button}<StatsTooltip
    {id}
    anchor={button}
    lines={[text]}
    onenter={interaction.keep}
    onleave={interaction.leave}
  />{/if}

<style>
  .help {
    display: inline-grid;
    place-items: center;
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    cursor: help;
  }
  .help:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  .help:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 1px;
  }
</style>
