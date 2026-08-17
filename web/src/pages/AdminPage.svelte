<script lang="ts">
  import { onMount } from "svelte";
  import {
    createInvitation,
    listAdminUsers,
    listAuditEvents,
    listInvitations,
    type AuditEvent,
    type Invitation
  } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatByteSize, formatDate } from "../format";
  import { holdNavigation } from "../navigation";
  import { showNotice } from "../notices";
  import { appState } from "../state";
  import type { AdminUser } from "../types";

  let users = $state<AdminUser[]>([]);
  let invitations = $state<Invitation[]>([]);
  let events = $state<AuditEvent[]>([]);
  let error = $state("");
  let creatingInvitation = $state(false);
  const initialLoadReady = holdNavigation();

  const pasteCount = $derived(users.reduce((total, user) => total + user.paste_count, 0));
  const storageBytes = $derived(users.reduce((total, user) => total + user.storage_bytes, 0));
  const activeSessions = $derived(users.reduce((total, user) => total + user.active_session_count, 0));
  const disabledUsers = $derived(users.filter(user => !user.enabled).length);
  const passwordResets = $derived(users.filter(user => user.password_change_required).length);
  const expiredInvitations = $derived(invitations.filter(invitation => invitation.status === "expired").length);
  const attentionCount = $derived(disabledUsers + passwordResets + expiredInvitations);
  const isOwner = $derived($appState.session.user?.role === "owner");

  onMount(() => {
    const requests: [Promise<AdminUser[]>, Promise<Invitation[]>, Promise<AuditEvent[]>] = [
      listAdminUsers(),
      listInvitations(),
      isOwner ? listAuditEvents() : Promise.resolve([])
    ];
    void Promise.all(requests)
      .then(([loadedUsers, loadedInvitations, loadedEvents]) => {
        users = loadedUsers;
        invitations = loadedInvitations;
        events = loadedEvents;
      })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load administration overview"; })
      .finally(initialLoadReady);
  });

  async function invite(): Promise<void> {
    creatingInvitation = true;
    try {
      const invitation = await createInvitation();
      await navigator.clipboard.writeText(new URL(invitation.url, location.origin).href);
      showNotice("Invitation link copied.");
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to create invitation", "error");
    } finally {
      creatingInvitation = false;
    }
  }
</script>

<section class="page-layout">
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Overview</h1></div></div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content admin-dashboard">
      {#if error}
        <section class="empty"><h2>Unable to load overview</h2><p>{error}</p></section>
      {:else}
        <section class="admin-summary" aria-label="Site summary">
          <article class="panel"><span>Users</span><strong>{users.length}</strong></article>
          <article class="panel"><span>Pastes</span><strong>{pasteCount}</strong></article>
          <article class="panel"><span>Stored data</span><strong>{formatByteSize(storageBytes)}</strong></article>
          <article class="panel"><span>Active sessions</span><strong>{activeSessions}</strong></article>
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>Attention</h2></div>{#if attentionCount}<span class="badge danger">{attentionCount}</span>{/if}</div>
          {#if attentionCount}
            <div class="data-list compact-data-list">
              {#if disabledUsers}<Link href="/admin/users" class="data-row"><span>Disabled users</span><strong>{disabledUsers}</strong></Link>{/if}
              {#if passwordResets}<Link href="/admin/users" class="data-row"><span>Password changes required</span><strong>{passwordResets}</strong></Link>{/if}
              {#if expiredInvitations}<Link href="/admin/invitations" class="data-row"><span>Expired invitations</span><strong>{expiredInvitations}</strong></Link>{/if}
            </div>
          {:else}
            <p class="dashboard-status"><Icon name="check"/> Everything looks normal.</p>
          {/if}
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>{isOwner ? "Recent activity" : "Recent accounts"}</h2></div></div>
          <div class="data-list compact-data-list">
            {#if isOwner}
              {#each events.slice(0, 5) as event (event.id)}
                <article class="data-row"><div><strong>{event.action.replaceAll(".", " ")}</strong><small>{event.actor_username} · {formatDate(event.created_at)}{event.target_label ? ` · ${event.target_label}` : ""}</small></div></article>
              {:else}<div class="empty"><p>No administrative activity yet.</p></div>{/each}
            {:else}
              {#each [...users].sort((left, right) => right.created_at - left.created_at).slice(0, 5) as user (user.id)}
                <Link class="data-row" href={`/admin/users/${user.id}`}><div><strong>{user.username}</strong><small>Joined {formatDate(user.created_at)}</small></div><span class="badge">{user.role === "admin" ? "Administrator" : "User"}</span></Link>
              {:else}<div class="empty"><p>No accounts yet.</p></div>{/each}
            {/if}
          </div>
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>Quick actions</h2></div></div>
          <div class="dashboard-actions">
            {#if $appState.config.invitations_enabled}<button class="button primary" type="button" disabled={creatingInvitation} onclick={invite}><Icon name="plus"/> Create invitation</button>{/if}
            <Link class="button" href="/admin/users"><Icon name="search"/> Find user</Link>
            <Link class="button" href="/admin/pastes"><Icon name="search"/> Search pastes</Link>
          </div>
        </section>
      {/if}
    </div>
  </div>
</section>
