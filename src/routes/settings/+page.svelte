<script lang="ts">
  import '../../app.css';
  import '$lib/styles/auxiliary-scrollbars.css';
  import { onMount, tick } from 'svelte';
  import {
    getSettings,
    getThemes,
    onSettingsChanged,
    onThemesChanged,
    auxiliaryWindowReady,
    onTimerPlansFocus,
    takeTimerPlansFocus,
    onCategoriesFocus,
    takeCategoriesFocus,
  } from '$lib/ipc';
  import { settings } from '$lib/stores/settings';
  import { applyTheme } from '$lib/stores/theme';
  import { resolveThemeName } from '$lib/utils/theme';
  import { setLocale } from '$lib/locale.svelte.js';
  import type { UnlistenFn } from '@tauri-apps/api/event';
  import { info, error as logError } from '@tauri-apps/plugin-log';
  import { createLocalShortcutHandler } from '$lib/utils/localShortcuts';

  import SettingsTitlebar from '$lib/components/settings/SettingsTitlebar.svelte';
  import CategorySection from '$lib/components/categories/CategorySection.svelte';
  import TimerSection from '$lib/components/settings/sections/TimerSection.svelte';
  import AppearanceSection from '$lib/components/settings/sections/AppearanceSection.svelte';
  import NotificationsSection from '$lib/components/settings/sections/NotificationsSection.svelte';
  import ShortcutsSection from '$lib/components/settings/sections/ShortcutsSection.svelte';
  import SystemSection from '$lib/components/settings/sections/SystemSection.svelte';
  import AboutSection from '$lib/components/settings/sections/AboutSection.svelte';

  import * as m from '$paraglide/messages.js';

  type Section =
    | 'timer'
    | 'categories'
    | 'appearance'
    | 'notifications'
    | 'shortcuts'
    | 'system'
    | 'about';

  const SECTIONS: { id: Section; label: () => string }[] = [
    { id: 'timer', label: m.nav_timer },
    { id: 'categories', label: m.category_title },
    { id: 'appearance', label: m.nav_appearance },
    { id: 'notifications', label: m.nav_notifications },
    { id: 'shortcuts', label: m.nav_shortcuts },
    { id: 'system', label: m.nav_system },
    { id: 'about', label: m.nav_about },
  ];

  let active = $state<Section>('timer');
  async function focusPlans() {
    if (!(await takeTimerPlansFocus())) return;
    active = 'timer';
    await tick();
    const field = document.getElementById('timer-plan-region');
    field?.scrollIntoView({ block: 'nearest' });
    field?.focus();
  }

  async function focusCategories() {
    if (!(await takeCategoriesFocus())) return;
    active = 'categories';
    await tick();
    const region = document.getElementById('category-region');
    region?.scrollIntoView({ block: 'nearest' });
    region?.focus();
  }

  // Local shortcut state for the settings window.
  let localVolume = $state(1.0);
  let preMuteVolume = $state(0.5);
  let isFullscreen = $state(false);

  onMount(() => {
    const cleanups: UnlistenFn[] = [];
    let disposed = false;
    onTimerPlansFocus(() => void focusPlans()).then((stop) => {
      if (disposed) stop();
      else cleanups.push(stop);
    });

    onCategoriesFocus(() => void focusCategories()).then((stop) => {
      if (disposed) stop();
      else cleanups.push(stop);
    });
    // Mount local keyboard shortcut handler.
    const shortcutHandler = createLocalShortcutHandler({
      getSettings: () => $settings,
      getVolume: () => localVolume,
      setVolume: (v) => {
        localVolume = v;
      },
      getPreMuteVolume: () => preMuteVolume,
      setPreMuteVolume: (v) => {
        preMuteVolume = v;
      },
      getFullscreen: () => isFullscreen,
      setFullscreen: (v) => {
        isFullscreen = v;
      },
    });
    document.addEventListener('keydown', shortcutHandler);
    cleanups.push(() => document.removeEventListener('keydown', shortcutHandler));

    (async () => {
      try {
        const s = await getSettings();
        settings.set(s);
        localVolume = s.volume;

        // Apply the stored locale on mount.
        setLocale(s.language);
        await info(`[settings] settings loaded, locale=${s.language}`);

        const themes = await getThemes();
        const osDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
        const activeTheme = themes.find((t) => t.name === resolveThemeName(s, osDark)) ?? themes[0];
        if (activeTheme) applyTheme(activeTheme);
        await info(`[settings] initialized, theme=${activeTheme?.name ?? 'none'}`);

        // Show the window now that the theme is applied (avoids white flash)
        await auxiliaryWindowReady();
        await focusPlans();
        await focusCategories();
      } catch (e) {
        await logError(`[settings] initialization failed: ${e}`);
        throw e;
      }

      // Live OS color scheme changes — re-resolve only in auto mode.
      const mq = window.matchMedia('(prefers-color-scheme: dark)');
      const mqListener = async (e: MediaQueryListEvent) => {
        if ($settings.theme_mode !== 'auto') return;
        const allThemes = await getThemes();
        const t = allThemes.find((th) => th.name === resolveThemeName($settings, e.matches));
        if (t) applyTheme(t);
      };
      mq.addEventListener('change', mqListener);
      cleanups.push(() => mq.removeEventListener('change', mqListener));

      cleanups.push(
        await onSettingsChanged(async (updated) => {
          const prevMode = $settings.theme_mode;
          const prevLight = $settings.theme_light;
          const prevDark = $settings.theme_dark;
          const prevLanguage = $settings.language;
          settings.set(updated);
          localVolume = updated.volume;
          if (updated.language !== prevLanguage) {
            setLocale(updated.language);
          }
          if (
            updated.theme_mode !== prevMode ||
            updated.theme_light !== prevLight ||
            updated.theme_dark !== prevDark
          ) {
            const allThemes = await getThemes();
            const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
            const t = allThemes.find((th) => th.name === resolveThemeName(updated, dark));
            if (t) applyTheme(t);
          }
        }),
        await onThemesChanged((updated) => {
          const dark = window.matchMedia('(prefers-color-scheme: dark)').matches;
          const current =
            updated.find((t) => t.name === resolveThemeName($settings, dark)) ?? updated[0];
          if (current) applyTheme(current);
        })
      );
    })();

    return () => {
      disposed = true;
      for (const fn of cleanups) fn();
    };
  });
</script>

<div class="window">
  <SettingsTitlebar />

  <div class="body">
    <!-- Left sidebar navigation -->
    <aside class="sidebar aux-scroll">
      <nav>
        {#each SECTIONS as section}
          <button
            class="nav-item"
            class:active={active === section.id}
            onclick={() => {
              active = section.id;
            }}
          >
            {section.label()}
          </button>
        {/each}
      </nav>
    </aside>

    <!-- Right content area -->
    <main class="content aux-scroll">
      <div class="reading-width">
        {#if active === 'timer'}
          <TimerSection />
        {:else if active === 'categories'}
          <CategorySection />
        {:else if active === 'appearance'}
          <AppearanceSection />
        {:else if active === 'notifications'}
          <NotificationsSection />
        {:else if active === 'shortcuts'}
          <ShortcutsSection />
        {:else if active === 'system'}
          <SystemSection />
        {:else if active === 'about'}
          <AboutSection />
        {/if}
      </div>
    </main>
  </div>
</div>

<style>
  .window {
    width: 100%;
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-background);
  }

  .body {
    min-width: 0;
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: hidden;
  }

  /* Sidebar */
  .sidebar {
    width: 180px;
    flex-shrink: 0;
    border-right: 1px solid var(--color-separator);
    background: var(--color-background-light);
    overflow-y: auto;
    padding: 8px 0;
  }

  .nav-item {
    display: block;
    width: 100%;
    padding: 9px 16px;
    background: none;
    border: none;
    border-left: 3px solid transparent;
    text-align: left;
    font-size: 0.85rem;
    letter-spacing: 0.03em;
    color: var(--color-foreground-darker, var(--color-foreground));
    cursor: pointer;
    transition:
      color 0.12s,
      background 0.12s,
      border-color 0.12s;
  }

  .nav-item:hover {
    color: var(--color-foreground);
    background: var(--color-hover);
  }

  .nav-item.active {
    color: var(--color-foreground);
    border-left-color: var(--color-accent);
    background: var(--color-hover);
    font-weight: 500;
  }

  /* Content */
  .content {
    min-height: 0;
    flex: 1;
    overflow-y: auto;
    min-width: 0;
  }
  .reading-width {
    width: 100%;
    max-width: 880px;
    min-width: 0;
    margin-inline: auto;
  }
  @media (max-width: 760px) {
    .sidebar {
      width: 156px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .nav-item {
      transition: none;
    }
  }
</style>
