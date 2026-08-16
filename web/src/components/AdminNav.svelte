<script lang="ts">
  import Link from "./Link.svelte";
  import { appState } from "../state";
  import { locationState } from "../navigation";

  const current = (path: string) => $locationState.path === path ? "page" : undefined;
</script>

<aside class="panel section-nav sticky-sidebar" aria-label="Administration">
  <Link href="/admin" aria-current={current("/admin")}>Overview</Link>
  <Link href="/admin/pastes" aria-current={current("/admin/pastes")}>Pastes</Link>
  <Link href="/admin/users" aria-current={$locationState.path.startsWith("/admin/users") ? "page" : undefined}>Users</Link>
  <Link href="/admin/invitations" aria-current={current("/admin/invitations")}>Invitations</Link>
  <Link href="/admin/api-keys" aria-current={current("/admin/api-keys")}>API keys</Link>
  {#if $appState.session.user?.role === "owner"}
    <span class="section-nav-heading">Owner</span>
    <Link href="/admin/settings" aria-current={current("/admin/settings")}>Settings</Link>
    <Link href="/admin/audit" aria-current={current("/admin/audit")}>Audit log</Link>
  {/if}
</aside>
