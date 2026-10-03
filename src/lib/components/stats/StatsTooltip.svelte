<script lang="ts">
  import { tick } from 'svelte';
  let {
    id,
    anchor,
    lines,
    onenter,
    onleave,
  }: {
    id: string;
    anchor: Element;
    lines: string[];
    onenter: () => void;
    onleave: () => void;
  } = $props();
  let element: HTMLDivElement;
  let position = $state({ left: 8, top: 8, ready: false });
  let frame = 0;

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }
  function place() {
    if (!element || !anchor.isConnected) return;
    // A stable category can outlive a zero/nonzero bar update. Resolve its current bar each time.
    const target =
      anchor.querySelector('[data-tooltip-anchor]') ??
      anchor.querySelector('[data-tooltip-hit]') ??
      anchor;
    const box = target.getBoundingClientRect();
    const tip = element.getBoundingClientRect();
    const above = box.top - tip.height - 8;
    position = {
      left: Math.max(
        8,
        Math.min(box.left + box.width / 2 - tip.width / 2, window.innerWidth - tip.width - 8)
      ),
      top: Math.max(
        8,
        Math.min(above >= 8 ? above : box.bottom + 8, window.innerHeight - tip.height - 8)
      ),
      ready: true,
    };
  }
  function queuePlace() {
    cancelAnimationFrame(frame);
    frame = requestAnimationFrame(place);
  }
  $effect(() => {
    anchor;
    lines;
    let active = true;
    tick().then(() => {
      if (active) place();
    });
    const observer = new ResizeObserver(queuePlace);
    observer.observe(element);
    observer.observe(anchor);
    if (anchor instanceof SVGElement && anchor.ownerSVGElement) {
      observer.observe(anchor.ownerSVGElement);
    }
    window.addEventListener('resize', queuePlace);
    window.addEventListener('scroll', queuePlace, true);
    return () => {
      active = false;
      cancelAnimationFrame(frame);
      observer.disconnect();
      window.removeEventListener('resize', queuePlace);
      window.removeEventListener('scroll', queuePlace, true);
    };
  });
</script>

<div
  use:portal
  bind:this={element}
  {id}
  role="tooltip"
  class="stats-tooltip"
  class:ready={position.ready}
  style:left="{position.left}px"
  style:top="{position.top}px"
  onpointerenter={onenter}
  onpointerleave={onleave}
>
  {#each lines as line}<div>{line}</div>{/each}
</div>

<style>
  .stats-tooltip {
    position: fixed;
    z-index: 9999;
    visibility: hidden;
    width: max-content;
    max-width: min(240px, calc(100vw - 16px));
    padding: 8px 10px;
    border-radius: 4px;
    background: var(--color-background-light);
    color: var(--color-foreground);
    border: 1px solid color-mix(in oklch, var(--color-foreground) 12%, transparent);
    box-shadow: 0 2px 8px color-mix(in oklch, var(--color-foreground) 15%, transparent);
    font-size: 0.72rem;
    line-height: 1.5;
    font-variant-numeric: tabular-nums;
    pointer-events: auto;
  }
  .ready {
    visibility: visible;
  }
  .stats-tooltip div:first-child {
    font-weight: 600;
    margin-bottom: 4px;
  }
</style>
