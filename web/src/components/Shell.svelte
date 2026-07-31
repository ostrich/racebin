<script lang="ts">
  import { requestApi } from "../api";
  import { appState } from "../state";
  import { replaceSession } from "../session";
  import { confirmDiscardChanges, guardUnsavedChanges, navigate } from "../router";
  import { notice } from "../notices";
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
    guardUnsavedChanges();
    await requestApi("/session", { method: "DELETE" });
    replaceSession({ authenticated: false });
    await navigate("/");
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
