<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
  import { error as logError } from '@tauri-apps/plugin-log';
  import * as m from '$paraglide/messages.js';
  let maximized = $state(false);
  onMount(() => {
    const window = getCurrentWebviewWindow();
    let disposed = false;
    let revision = 0;
    async function update() {
      const request = ++revision;
      try {
        const value = await window.isMaximized();
        if (!disposed && request === revision) maximized = value;
      } catch (error) {
        if (!disposed) void logError(`[aux-window] state: ${error}`);
      }
    }
    void update();
    const stop = window.onResized(() => void update());
    return () => {
      disposed = true;
      void stop.then((unlisten) => unlisten());
    };
  });
</script>

<div class="window-controls">
  <button
    onclick={() => getCurrentWebviewWindow().toggleMaximize()}
    aria-label={maximized ? m.window_restore() : m.window_maximize()}
  >
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
      {#if maximized}<path d="M4 3V1h7v7H9M1 4h7v7H1Z" stroke="currentColor" stroke-width="1.2" />
      {:else}<rect
          x="1.5"
          y="1.5"
          width="9"
          height="9"
          stroke="currentColor"
          stroke-width="1.2"
        />{/if}
    </svg>
  </button>
  <button
    class="close"
    onclick={() => getCurrentWebviewWindow().close()}
    aria-label={m.window_close()}
  >
    <svg width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true"
      ><path d="m1 1 10 10M11 1 1 11" stroke="currentColor" stroke-width="1.5" /></svg
    >
  </button>
</div>

<style>
  .window-controls {
    position: absolute;
    right: 8px;
    display: flex;
    gap: 4px;
  }
  button {
    width: 28px;
    height: 28px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--color-foreground-darker);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition:
      color 120ms,
      background 120ms;
  }
  button:hover {
    background: var(--color-hover);
    color: var(--color-foreground);
  }
  .close:hover {
    color: var(--color-background);
    background: var(--color-focus-round);
  }
  button:focus-visible {
    outline: 2px solid color-mix(in oklch, var(--color-foreground) 45%, transparent);
    outline-offset: 2px;
  }
  @media (prefers-reduced-motion: reduce) {
    button {
      transition: none;
    }
  }
</style>
