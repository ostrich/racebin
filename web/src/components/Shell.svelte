<script lang="ts">
  import { onMount } from "svelte";
  import { isSessionInvalidError, logout as logoutSession } from "../api";
  import { appState } from "../app/state";
  import { replaceSession } from "../app/session";
  import { confirmDiscardChanges, navigate } from "../navigation";
  import { clearNotice, notice, showNotice } from "../app/notices";
  import { setColorTheme, uiPreferences, type ColorTheme } from "../app/uiPreferences";
  import Icon from "./Icon.svelte";
  import type { IconName } from "./icons";
  import Link from "./Link.svelte";

  let {
    children,
    minimal = false
  }: {
    children: import("svelte").Snippet;
    minimal?: boolean;
  } = $props();

  async function logout(): Promise<void> {
    if (!(await confirmDiscardChanges())) return;
    try {
      await logoutSession();
    } catch (error) {
      if (!isSessionInvalidError(error)) {
        showNotice(error instanceof Error ? error.message : "Unable to log out", "error");
        return;
      }
    }
    replaceSession({ authenticated: false, permissions: [] });
    await navigate("/", { discardConfirmed: true });
    clearNotice();
  }

  const themes: Record<ColorTheme, { next: ColorTheme; label: string; icon: IconName }> = {
    auto: { next: "dark", label: "Automatic theme", icon: "monitor" },
    dark: { next: "light", label: "Dark theme", icon: "moon" },
    light: { next: "auto", label: "Light theme", icon: "sun" }
  };
  let currentTheme = $derived(themes[$uiPreferences.colorTheme]);
  let showBackToTop = $state(false);

  function updateBackToTop(): void {
    showBackToTop =
      window.scrollY >= window.innerHeight &&
      document.documentElement.scrollHeight > window.innerHeight;
  }

  function backToTop(): void {
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    window.scrollTo({ top: 0, behavior: reduceMotion ? "auto" : "smooth" });
  }

  onMount(() => {
    window.addEventListener("scroll", updateBackToTop, { passive: true });
    window.addEventListener("resize", updateBackToTop, { passive: true });
    updateBackToTop();
    return () => {
      window.removeEventListener("scroll", updateBackToTop);
      window.removeEventListener("resize", updateBackToTop);
    };
  });
</script>

<header class="site-header">
  <Link class="brand" href="/">{$appState.config.site_name}</Link>
  {#if !minimal}
    <nav class="primary-nav">
      {#if $appState.config.public_explore_enabled}<Link href="/explore">Explore</Link>{/if}
      {#if $appState.session.user}
        <Link href="/pastes">My pastes</Link>
        <Link href="/pastes/new"><Icon name="plus" /> New</Link>
        <Link href="/help">Help</Link>
      {/if}
      {#if $appState.session.user?.role === "admin" || $appState.session.user?.role === "owner"}
        <Link href="/admin">Admin</Link>
      {/if}
    </nav>
    <div class="session">
      <button
        class="theme-control"
        type="button"
        title={`${currentTheme.label}; click for ${themes[currentTheme.next].label.toLowerCase()}`}
        aria-label={`Color theme: ${currentTheme.label}`}
        onclick={() => setColorTheme(currentTheme.next)}><Icon name={currentTheme.icon} /></button
      >
      {#if $appState.session.user}
        <Link href="/account"
          ><Icon name="user-round" /><span>{$appState.session.user.username}</span></Link
        >
        <button
          class="icon-button"
          type="button"
          title="Log out"
          aria-label="Log out"
          onclick={logout}
        >
          <Icon name="log-out" />
        </button>
      {:else}
        <Link href="/login"><Icon name="log-in" /><span>Log in</span></Link>
      {/if}
    </div>
  {/if}
</header>
<main>{@render children()}</main>
<div class="fixed-utilities">
  {#if $notice}<div
      class="toast show"
      class:error={$notice.variant === "error"}
      role="status"
      aria-live="polite"
    >
      {$notice.message}
    </div>{/if}
  <button
    class="page-start-control icon-button"
    class:visible={showBackToTop}
    type="button"
    title="Jump to page start"
    aria-label="Jump to page start"
    aria-hidden={!showBackToTop}
    tabindex={showBackToTop ? 0 : -1}
    onclick={backToTop}
  >
    <Icon name="arrow-up" />
  </button>
</div>
