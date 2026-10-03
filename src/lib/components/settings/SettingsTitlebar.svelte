<script lang="ts">
  import AuxiliaryWindowControls from '$lib/components/AuxiliaryWindowControls.svelte';
  import { isMac } from '$lib/utils/platform';
  import * as m from '$paraglide/messages.js';
</script>

<nav class="titlebar" class:macos={isMac} data-tauri-drag-region>
  <h1 class="title">{m.settings_title()}</h1>
  <!-- Hidden on macOS; the native traffic light close button handles this. -->
  {#if !isMac}
    <AuxiliaryWindowControls />
  {/if}
</nav>

<style>
  .titlebar {
    height: 40px;
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    position: relative;
    flex-shrink: 0;
    border-bottom: 1px solid var(--color-separator);
  }

  /* Shift the centered title right of the traffic lights on macOS. */
  .macos {
    padding-left: 72px;
  }

  .title {
    font-size: 0.8rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-foreground-darker, var(--color-foreground));
    pointer-events: none;
  }
</style>
