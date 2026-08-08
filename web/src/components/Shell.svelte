<script lang="ts">
  import { logout as logoutSession } from "../api";
  import { appState } from "../state";
  import { replaceSession } from "../session";
  import { clearUnsavedChangesGuard, confirmDiscardChanges, navigate } from "../navigation";
  import { notice } from "../notices";
  import { setColorTheme, uiPreferences, type ColorTheme } from "../uiPreferences";
  import Icon from "./Icon.svelte";
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
    clearUnsavedChangesGuard();
    await logoutSession();
    replaceSession({ authenticated: false });
    await navigate("/");
  }

  const themes: Record<ColorTheme, { next: ColorTheme; label: string; icon: string }> = {
    auto: { next: "dark", label: "Automatic theme", icon: "monitor" },
    dark: { next: "light", label: "Dark theme", icon: "moon" },
    light: { next: "auto", label: "Light theme", icon: "sun" }
  };
  let currentTheme = $derived(themes[$uiPreferences.colorTheme]);
</script>

<header>
  <Link class="brand" href="/">{$appState.config.site_name}</Link>
  {#if !minimal}
    <nav>
      <Link href="/explore">Explore</Link>
      {#if $appState.session.user}
        <Link href="/pastes">My pastes</Link>
        <Link href="/pastes/new"><Icon name="plus"/> New</Link>
        <Link href="/help">Help</Link>
      {/if}
      {#if $appState.session.user?.role === "admin"}
        <Link href="/admin">Admin</Link>
      {/if}
    </nav>
    <div class="session">
      <button class="theme-control" type="button"
        title={`${currentTheme.label}; click for ${themes[currentTheme.next].label.toLowerCase()}`}
        aria-label={`Color theme: ${currentTheme.label}`}
        onclick={() => setColorTheme(currentTheme.next)}><Icon name={currentTheme.icon}/></button>
      {#if $appState.session.user}
        <Link href="/account"><Icon name="user-round"/><span>{$appState.session.user.username}</span></Link>
        <button class="icon-button" type="button" title="Log out" aria-label="Log out" onclick={logout}>
          <Icon name="log-out"/>
        </button>
      {:else}
        <Link href="/login"><Icon name="log-in"/><span>Log in</span></Link>
      {/if}
    </div>
  {/if}
</header>
<main>{@render children()}</main>
<div id="toast" class:show={$notice} class:error={$notice?.variant === "error"}
  role="status" aria-live="polite">{$notice?.message ?? ""}</div>
