<script lang="ts">
  import { onMount } from "svelte";
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatByteSize, formatDate } from "../format";
  import { showNotice } from "../notices";
  import { deferRouteReady } from "../router";
  import type { AdminUser } from "../types";

  let { userId }: { userId: number } = $props();
  let user = $state<AdminUser | null>(null);
  let role = $state<"user" | "admin">("user");
  let error = $state("");
  let busy = $state(false);

  async function load(): Promise<void> {
    user = await requestApi<AdminUser>(`/admin/users/${userId}`);
    role = user.role;
  }
  onMount(() => { const ready = deferRouteReady(); void load().catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load user"; }).finally(ready); });

  async function patch(values: { role?: string; enabled?: boolean }): Promise<void> {
    if (!user) return;
    busy = true;
    try {
      await requestApi(`/admin/users/${user.id}`, { method: "PATCH", body: JSON.stringify(values) });
      await load();
      showNotice("Account updated.");
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to update account", "error"); await load(); }
    finally { busy = false; }
  }

  async function resetLink(): Promise<void> {
    if (!user) return;
    try {
      const result = await requestApi<{ url: string }>(`/admin/users/${user.id}/password-reset`, { method: "POST" });
      await navigator.clipboard.writeText(new URL(result.url, location.origin).href);
      showNotice("Password reset link copied.");
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to create reset link", "error"); }
  }

  async function revoke(path: "sessions" | "api-keys", label: string): Promise<void> {
    if (!user || !confirm(`${label} for ${user.username}?`)) return;
    try { await requestApi(`/admin/users/${user.id}/${path}`, { method: "DELETE" }); await load(); showNotice(`${label} completed.`); }
    catch (reason) { showNotice(reason instanceof Error ? reason.message : `Unable to ${label.toLowerCase()}`, "error"); }
  }

  function toggleEnabled(): void {
    if (!user) return;
    if (user.enabled && !confirm(`Disable ${user.username}? Their sessions will be revoked.`)) return;
    void patch({ enabled: !user.enabled });
  }
</script>

<section class="stack page-stack">
  {#if error}<section class="empty"><h1>Unable to load user</h1><p>{error}</p><Link class="button" href="/admin/users">Back to users</Link></section>
  {:else if !user}<p class="muted">Loading user…</p>
  {:else}
    <div class="page-heading"><div><p class="eyebrow"><Link href="/admin/users">Users</Link></p><h1>{user.username}</h1><div class="badge-group"><span class="badge">{user.role === "admin" ? "Administrator" : "User"}</span><span class:danger={!user.enabled} class="badge">{user.enabled ? "Enabled" : "Disabled"}</span></div></div><Link class="button" href={`/admin/pastes?owner_id=${user.id}`}>View pastes</Link></div>
    <div class="admin-user-metrics">
      <article class="panel"><span>Pastes</span><strong>{user.paste_count}</strong><small>{formatByteSize(user.storage_bytes)} stored</small></article>
      <article class="panel"><span>Sessions</span><strong>{user.active_session_count}</strong><small>active</small></article>
      <article class="panel"><span>API keys</span><strong>{user.active_api_key_count}</strong><small>{user.api_key_count} total</small></article>
      <article class="panel"><span>Last login</span><strong class="metric-date">{user.last_login_at ? formatDate(user.last_login_at) : "Never"}</strong><small>Joined {formatDate(user.created_at)}</small></article>
    </div>
    <div class="admin-user-panels">
      <section class="panel stack"><h2>Account access</h2><p class="muted">Change the user's role or prevent the account from signing in.</p>
        <div class="admin-user-access"><label class="field"><span>Role</span><select bind:value={role}><option value="user">User</option><option value="admin">Administrator</option></select></label><button class="button" type="button" disabled={busy || role === user.role} onclick={() => patch({ role })}>Save role</button></div>
        <button class:danger={user.enabled} class="button" type="button" disabled={busy} onclick={toggleEnabled}>{user.enabled ? "Disable account" : "Enable account"}</button>
      </section>
      <section class="panel stack"><h2>Security</h2><p class="muted">Recovery links are valid once for one hour. Resetting a password signs out every existing session.</p>
        <div class="admin-security-actions"><button class="button" type="button" onclick={resetLink}><Icon name="copy"/> Create and copy reset link</button><button class="button" type="button" onclick={() => revoke("sessions", "Sign out all sessions")}>Sign out everywhere</button><button class="button danger" type="button" onclick={() => revoke("api-keys", "Revoke all API keys")}>Revoke all API keys</button></div>
      </section>
    </div>
  {/if}
</section>
