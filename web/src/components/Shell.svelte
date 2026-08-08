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

  function changeTheme(event: Event): void {
    setColorTheme((event.currentTarget as HTMLSelectElement).value as ColorTheme);
  }
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
      <label class="theme-control" title="Color theme">
        <span class="visually-hidden">Color theme</span>
        <select aria-label="Color theme" value={$uiPreferences.colorTheme} onchange={changeTheme}>
          <option value="auto">Auto</option>
          <option value="dark">Dark</option>
          <option value="light">Light</option>
        </select>
      </label>
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
