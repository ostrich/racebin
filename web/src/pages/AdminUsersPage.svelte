<script lang="ts">
  import { onMount } from "svelte";
  import { createInvitation as createInvitationRequest, listAdminUsers } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatByteSize, formatDate } from "../format";
  import { showNotice } from "../notices";
  import { holdNavigation } from "../navigation";
  import type { AdminUser } from "../types";

  let users = $state<AdminUser[]>([]);
  let search = $state("");
  let role = $state("");
  let status = $state("");
  let sort = $state("username");
  let error = $state("");
  const initialLoadReady = holdNavigation();
  let filtered = $derived(users.filter(user =>
    (!search || user.username.toLowerCase().includes(search.toLowerCase())) &&
    (!role || user.role === role) &&
    (!status || (status === "enabled") === user.enabled)
  ).sort((left, right) => {
    if (sort === "created") return right.created_at - left.created_at;
    if (sort === "login") return (right.last_login_at ?? 0) - (left.last_login_at ?? 0);
    if (sort === "pastes") return right.paste_count - left.paste_count;
    if (sort === "storage") return right.storage_bytes - left.storage_bytes;
    return left.username.localeCompare(right.username);
  }));

  onMount(() => {
    void listAdminUsers()
      .then(value => { users = value; })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load users"; })
      .finally(initialLoadReady);
  });

  async function createInvitation(): Promise<void> {
    try {
      const invitation = await createInvitationRequest();
      await navigator.clipboard.writeText(new URL(invitation.url, location.origin).href);
      showNotice("Invitation link copied.");
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to create invitation", "error"); }
  }
</script>

<section class="stack page-stack">
  <div class="page-heading">
    <div><p class="eyebrow"><Link href="/admin">Administration</Link></p><h1>Users</h1><p class="muted">Manage account access and recovery.</p></div>
    <button class="button primary" type="button" onclick={createInvitation}><Icon name="plus"/> Create invitation</button>
  </div>
  <div class="panel admin-user-filters">
    <label class="field search-control"><span>Search</span><input type="search" placeholder="Username" bind:value={search}></label>
    <label class="field"><span>Role</span><select bind:value={role}><option value="">Any role</option><option value="user">User</option><option value="admin">Administrator</option></select></label>
    <label class="field"><span>Status</span><select bind:value={status}><option value="">Any status</option><option value="enabled">Enabled</option><option value="disabled">Disabled</option></select></label>
    <label class="field"><span>Sort</span><select bind:value={sort}><option value="username">Username</option><option value="created">Newest</option><option value="login">Last login</option><option value="pastes">Paste count</option><option value="storage">Storage</option></select></label>
  </div>
  {#if error}<section class="empty"><h2>Unable to load users</h2><p>{error}</p></section>
  {:else}<div class="panel admin-user-table" role="table" aria-label="Users">
    <div class="admin-user-row admin-user-header" role="row"><span>User</span><span>Access</span><span>Activity</span><span>Usage</span><span></span></div>
    {#each filtered as user (user.id)}
      <div class="admin-user-row" role="row">
        <div><Link href={`/admin/users/${user.id}`}><strong>{user.username}</strong></Link><small>Joined {formatDate(user.created_at)}</small></div>
        <div class="badge-group"><span class="badge">{user.role === "admin" ? "Administrator" : "User"}</span><span class:danger={!user.enabled} class="badge">{user.enabled ? "Enabled" : "Disabled"}</span></div>
        <div><span>{user.last_login_at ? formatDate(user.last_login_at) : "Never logged in"}</span><small>{user.active_session_count} active {user.active_session_count === 1 ? "session" : "sessions"}</small></div>
        <div><span>{user.paste_count} {user.paste_count === 1 ? "paste" : "pastes"}</span><small>{formatByteSize(user.storage_bytes)} · {user.active_api_key_count} active keys</small></div>
        <Link class="button" href={`/admin/users/${user.id}`}>Manage</Link>
      </div>
    {:else}<div class="empty"><p>No users match these filters.</p></div>{/each}
  </div>{/if}
</section>
