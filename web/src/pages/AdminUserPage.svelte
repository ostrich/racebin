<script lang="ts">
  import { onMount } from "svelte";
  import {
    createPasswordReset, getAdminUser, revokeUserApiKeys, revokeUserSessions,
    updateAdminUser, updateAdminUserRole, transferOwnership, reauthenticate, type UserUpdate
  } from "../api";
  import Icon from "../components/Icon.svelte";
  import AdminNav from "../components/AdminNav.svelte";
  import PasswordConfirmDialog from "../components/PasswordConfirmDialog.svelte";
  import Link from "../components/Link.svelte";
  import { formatByteSize, formatDate } from "../format";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import type { AdminUser } from "../types";
  import { appState } from "../state";

  let { userId }: { userId: number } = $props();
  const initialLoadReady = holdNavigation();
  let user = $state<AdminUser | null>(null);
  let role = $state<"user" | "admin">("user");
  let error = $state("");
  let busy = $state(false);
  let passwordDialog: PasswordConfirmDialog;
  let canManage = $derived(user?.role === "user" || $appState.session.user?.role === "owner");

  async function load(): Promise<void> {
    user = await getAdminUser(userId);
    role = user.role === "owner" ? "admin" : user.role;
  }
  onMount(() => { void load().catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load user"; }).finally(initialLoadReady); });

  async function patch(values: UserUpdate): Promise<void> {
    if (!user) return;
    busy = true;
    try {
      await updateAdminUser(user.id, values);
      await load();
      showNotice("Account updated.");
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to update account", "error"); await load(); }
    finally { busy = false; }
  }

  async function resetLink(): Promise<void> {
    if (!user) return;
    try {
      const result = await createPasswordReset(user.id);
      await navigator.clipboard.writeText(new URL(result.url, location.origin).href);
      showNotice("Password reset link copied.");
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to create reset link", "error"); }
  }

  async function revoke(kind: "sessions" | "api-keys", label: string): Promise<void> {
    if (!user || !confirm(`${label} for ${user.username}?`)) return;
    try {
      await (kind === "sessions" ? revokeUserSessions(user.id) : revokeUserApiKeys(user.id));
      await load(); showNotice(`${label} completed.`);
    }
    catch (reason) { showNotice(reason instanceof Error ? reason.message : `Unable to ${label.toLowerCase()}`, "error"); }
  }

  function toggleEnabled(): void {
    if (!user) return;
    if (user.enabled && !confirm(`Disable ${user.username}? Their sessions will be revoked.`)) return;
    void patch({ enabled: !user.enabled });
  }

  async function confirmPassword(): Promise<boolean> {
    const password = await passwordDialog.ask();
    if (!password) return false;
    try { await reauthenticate(password); return true; }
    catch (reason) { showNotice(reason instanceof Error ? reason.message : "Password confirmation failed", "error"); return false; }
  }

  async function saveRole(): Promise<void> {
    if (!user || !(await confirmPassword())) return;
    try { await updateAdminUserRole(user.id, role); await load(); showNotice("Role updated."); }
    catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to update role", "error"); }
  }

  async function makeOwner(): Promise<void> {
    if (!user || !confirm(`Transfer site ownership to ${user.username}?` ) || !(await confirmPassword())) return;
    try { await transferOwnership(user.id); location.assign("/admin"); }
    catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to transfer ownership", "error"); }
  }
</script>

<PasswordConfirmDialog bind:this={passwordDialog}/>

<section class="page-layout">
  {#if error}<section class="empty"><h1>Unable to load user</h1><p>{error}</p><Link class="button" href="/admin/users">Back to users</Link></section>
  {:else if !user}<p class="muted">Loading user…</p>
  {:else}
    <div class="page-heading"><div><p class="eyebrow"><Link href="/admin/users">Users</Link></p><h1>{user.username}</h1><div class="badge-group"><span class="badge">{user.role === "owner" ? "Owner" : user.role === "admin" ? "Administrator" : "User"}</span><span class:danger={!user.enabled} class="badge">{user.enabled ? "Enabled" : "Disabled"}</span></div></div><Link class="button" href={`/admin/pastes?owner_id=${user.id}`}>View pastes</Link></div>
    <div class="section-layout"><AdminNav/><div class="section-content">
    <div class="admin-user-metrics">
      <article class="panel"><span>Pastes</span><strong>{user.paste_count}</strong><small>{formatByteSize(user.storage_bytes)} stored</small></article>
      <article class="panel"><span>Sessions</span><strong>{user.active_session_count}</strong><small>active</small></article>
      <article class="panel"><span>API keys</span><strong>{user.active_api_key_count}</strong><small>{user.api_key_count} total</small></article>
      <article class="panel"><span>Last login</span><strong class="metric-date">{user.last_login_at ? formatDate(user.last_login_at) : "Never"}</strong><small>Joined {formatDate(user.created_at)}</small></article>
    </div>
    <div class="admin-user-panels">
      <section class="panel stack"><h2>Account access</h2><p class="muted">Control whether this account can sign in.</p>
        {#if $appState.session.user?.role === "owner" && user.role !== "owner"}<div class="admin-user-access"><label class="field"><span>Role</span><select bind:value={role}><option value="user">User</option><option value="admin">Administrator</option></select></label><button class="button" type="button" disabled={busy || role === user.role} onclick={saveRole}>Save role</button></div>{/if}
        {#if !canManage}<p class="notice">Only the owner can change an administrator account.</p>{/if}
        <button class:danger={user.enabled} class="button" type="button" disabled={busy || !canManage} onclick={toggleEnabled}>{user.enabled ? "Disable account" : "Enable account"}</button>
        {#if $appState.session.user?.role === "owner" && user.role === "admin"}<button class="button danger" type="button" onclick={makeOwner}>Transfer ownership to this administrator</button>{/if}
      </section>
      <section class="panel stack"><h2>Security</h2><p class="muted">Recovery links are valid once for one hour. Resetting a password signs out every existing session.</p>
        <div class="admin-security-actions"><button class="button" type="button" disabled={!canManage} onclick={resetLink}><Icon name="copy"/> Create and copy reset link</button><button class="button" type="button" disabled={!canManage} onclick={() => revoke("sessions", "Sign out all sessions")}>Sign out everywhere</button><button class="button danger" type="button" disabled={!canManage} onclick={() => revoke("api-keys", "Revoke all API keys")}>Revoke all API keys</button></div>
      </section>
    </div>
    </div></div>
  {/if}
</section>
