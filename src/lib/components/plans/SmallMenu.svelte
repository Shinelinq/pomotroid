<script lang="ts">
  import { onMount, tick, type Snippet } from 'svelte';
  let {
    label,
    title = label,
    children,
    icon = false,
    followLayout = false,
  }: {
    label: string;
    title?: string;
    children: Snippet;
    icon?: boolean;
    followLayout?: boolean;
  } = $props();
  const id = $props.id();
  let trigger: HTMLButtonElement;
  let panel: HTMLDivElement;
  let opened = $state(false);
  let left = $state(0);
  let top = $state(0);
  function place() {
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(260, window.innerWidth - 16);
    left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
    const height = Math.min(
      followLayout ? panel.getBoundingClientRect().height : panel.scrollHeight,
      window.innerHeight - 16
    );
    top =
      rect.bottom + height + 4 <= window.innerHeight - 8
        ? rect.bottom + 4
        : Math.max(8, rect.top - height - 4);
  }
  export function close() {
    panel?.hidePopover();
    trigger?.focus();
  }
  export function closeIfOpen() {
    if (!opened) return false;
    close();
    return true;
  }
  async function toggled(event: ToggleEvent) {
    opened = event.newState === 'open';
    if (opened) {
      await tick();
      place();
      panel.querySelector<HTMLElement>('button:not(:disabled)')?.focus();
    }
  }
  function keys(event: KeyboardEvent) {
    event.stopPropagation();
    const items = [...panel.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const index = items.indexOf(document.activeElement as HTMLButtonElement);
    let next = index;
    if (event.key === 'ArrowDown') next = (index + 1) % items.length;
    else if (event.key === 'ArrowUp') next = (index - 1 + items.length) % items.length;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = items.length - 1;
    else if (event.key === 'Escape') {
      event.preventDefault();
      close();
      return;
    } else if (event.key === 'Tab') {
      panel.hidePopover();
      return;
    } else return;
    event.preventDefault();
    items[next]?.focus();
  }
  onMount(() => {
    let frame = 0;
    const resize = () => {
      if (!followLayout) {
        if (opened) place();
        return;
      }
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        frame = requestAnimationFrame(() => {
          if (opened) place();
        });
      });
    };
    const observer = followLayout ? new ResizeObserver(resize) : undefined;
    observer?.observe(trigger);
    window.addEventListener('resize', resize);
    return () => {
      observer?.disconnect();
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', resize);
    };
  });
</script>

<button
  bind:this={trigger}
  class="trigger"
  class:icon
  {title}
  aria-label={label}
  aria-haspopup="menu"
  aria-expanded={opened}
  popovertarget={id}
  onkeydown={(event) => event.stopPropagation()}
>
  {#if icon}<svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true"
      ><circle cx="3" cy="8" r="1" fill="currentColor" /><circle
        cx="8"
        cy="8"
        r="1"
        fill="currentColor"
      /><circle cx="13" cy="8" r="1" fill="currentColor" /></svg
    >
  {:else}<span>{label}</span><svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"
      ><path d="m2 4 3 3 3-3" fill="none" stroke="currentColor" /></svg
    >{/if}
</button>
<div
  {id}
  bind:this={panel}
  popover="auto"
  role="menu"
  tabindex="-1"
  aria-label={label}
  class="panel"
  style:left="{left}px"
  style:top="{top}px"
  ontoggle={toggled}
  onkeydown={keys}
>
  {@render children()}
</div>

<style>
  .trigger {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
    height: 28px;
    padding: 0 6px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .trigger span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .trigger svg {
    flex: none;
  }
  .trigger.icon {
    width: 28px;
    justify-content: center;
  }
  .trigger:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  .trigger:focus-visible,
  .panel :global(button:focus-visible) {
    outline: 1px solid var(--color-foreground);
    outline-offset: -2px;
  }
  .panel {
    position: fixed;
    margin: 0;
    width: 260px;
    max-width: calc(100vw - 16px);
    max-height: calc(100vh - 16px);
    overflow-y: auto;
    padding: 4px;
    border: 1px solid var(--color-separator);
    border-radius: 4px;
    color: var(--color-foreground);
    background: var(--color-background-light);
    font-size: 12px;
  }
  .panel :global(button) {
    width: 100%;
    min-height: 30px;
    padding: 6px 8px;
    display: block;
    text-align: left;
    background: transparent;
    border: 0;
    border-radius: 4px;
    color: inherit;
    font: inherit;
    cursor: pointer;
  }
  .panel :global(button:hover:not(:disabled)) {
    background: var(--color-hover);
  }
  .panel :global(button:disabled) {
    opacity: 0.55;
    cursor: default;
  }
  .panel :global(small) {
    display: block;
    margin-top: 3px;
    color: var(--color-foreground-darker);
    font-size: 11px;
    line-height: 1.4;
  }
  .panel :global(hr) {
    border: 0;
    border-top: 1px solid var(--color-separator);
    margin: 4px 0;
  }
  .panel :global(.menu-note) {
    padding: 6px 8px;
    font-size: 11px;
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
</style>
