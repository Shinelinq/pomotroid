<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { ReportScope } from '$lib/data/types';
  import ReportForm from './ReportForm.svelte';
  import * as m from '$paraglide/messages.js';
  let { source, onclose }: { source: ReportScope; onclose: () => void } = $props();
  let dialog: HTMLDialogElement;
  let busy = $state(false);
  onMount(() => {
    dialog.showModal();
    const stop = getCurrentWindow().onCloseRequested((event) => {
      if (busy) event.preventDefault();
    });
    return () => {
      dialog.close();
      void stop.then((fn) => fn());
    };
  });
</script>

<dialog
  bind:this={dialog}
  class="aux-scroll"
  aria-label={m.data_csv()}
  oncancel={(e) => {
    e.preventDefault();
    if (!busy) onclose();
  }}
>
  <ReportForm {source} {onclose} onbusy={(value) => (busy = value)} />
</dialog>

<style>
  dialog {
    margin: auto;
    width: 480px;
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 32px);
    box-sizing: border-box;
    border: 1px solid var(--color-separator);
    border-radius: 4px;
    background: var(--color-background);
    color: var(--color-foreground);
    padding: 20px;
    overflow-y: auto;
  }
  dialog::backdrop {
    background: color-mix(in srgb, var(--color-background) 60%, transparent);
  }
</style>
