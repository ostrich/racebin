<script lang="ts">
  import { onMount } from "svelte";
  import {
    listAdminUsers,
    listAuditEvents,
    getAdminSummary,
    type AdminSummary,
    type AuditEvent,
  } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import InvitationDialog from "../components/InvitationDialog.svelte";
  import Link from "../components/Link.svelte";
  import { formatByteSize, formatDate } from "../format";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { AdminUser } from "../types";

  let summary = $state<AdminSummary | null>(null);
  let recentUsers = $state<AdminUser[]>([]);
  let events = $state<AuditEvent[]>([]);
  let error = $state("");
  let invitationDialog: InvitationDialog;
  const initialLoadReady = holdNavigation();

  const isOwner = $derived($appState.session.user?.role === "owner");
  const passwordResets = $derived(summary?.password_change_required_count ?? 0);
  const expiringInvitations = $derived(summary?.expiring_invitation_count ?? 0);
  const invitationsUnavailable = $derived(isOwner && !$appState.config.invitations_enabled && (summary?.active_invitation_count ?? 0) > 0);
  const actionCount = $derived(passwordResets + expiringInvitations + (invitationsUnavailable ? 1 : 0));

  onMount(() => {
    const requests: [Promise<AdminSummary>, Promise<AdminUser[]>, Promise<AuditEvent[]>] = [
      getAdminSummary(),
      listAdminUsers(new URLSearchParams({ sort: "created", direction: "desc", page_size: "5" })).then(page => page.items),
      isOwner ? listAuditEvents(new URLSearchParams({ page_size: "5" })).then(page => page.items) : Promise.resolve([])
    ];
    void Promise.all(requests)
      .then(([loadedSummary, loadedUsers, loadedEvents]) => {
        summary = loadedSummary;
        recentUsers = loadedUsers;
        events = loadedEvents;
      })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load administration overview"; })
      .finally(initialLoadReady);
  });

  async function reloadInvitations(): Promise<void> {
    summary = await getAdminSummary();
  }

  function activityLabel(action: string): string {
    const labels: Record<string, string> = {
      "invitation.created": "Created an invitation",
      "invitation.revoked": "Revoked an invitation",
      "invitation.comment_updated": "Updated an invitation note",
      "user.password_reset_created": "Created a password reset link",
      "user.sessions_revoked": "Signed a user out everywhere",
      "user.api_keys_revoked": "Revoked a user’s API keys",
      "user.updated": "Updated a user account",
      "user.role_updated": "Changed a user role",
      "ownership.transferred": "Transferred site ownership",
      "instance.settings_changed": "Changed site settings"
    };
    return labels[action] ?? action.replaceAll(".", " ");
  }

  function activityUrl(event: AuditEvent): string | null {
    if (event.target_type === "user" && event.target_id) return `/admin/users/${event.target_id}`;
    if (event.target_type === "paste" && event.target_id) return `/pastes/${event.target_id}`;
    if (event.target_type === "invitation") return "/admin/invitations?view=history";
    return null;
  }
</script>

<InvitationDialog bind:this={invitationDialog} oncreated={reloadInvitations}/>

<section class="page-layout">
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Overview</h1></div></div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content admin-dashboard">
      {#if error}
        <section class="empty"><h2>Unable to load overview</h2><p>{error}</p></section>
      {:else}
        <section class="admin-summary" aria-label="Site summary">
          <article class="panel"><span>Users</span><strong>{summary?.user_count ?? 0}</strong></article>
          <article class="panel"><span>Pastes</span><strong>{summary?.paste_count ?? 0}</strong></article>
          <article class="panel"><span>Stored data</span><strong>{formatByteSize(summary?.storage_bytes ?? 0)}</strong></article>
          <article class="panel"><span>Active sessions</span><strong>{summary?.active_session_count ?? 0}</strong></article>
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>Needs action</h2></div>{#if actionCount}<span class="badge danger">{actionCount}</span>{/if}</div>
          {#if actionCount}
            <div class="data-list compact-data-list">
              {#if passwordResets}<Link href="/admin/users" class="data-row"><span>Password changes required</span><strong>{passwordResets}</strong></Link>{/if}
              {#if expiringInvitations}<Link href="/admin/invitations" class="data-row"><span>Invitations expiring within four hours</span><strong>{expiringInvitations}</strong></Link>{/if}
              {#if invitationsUnavailable}<Link href="/admin/settings" class="data-row"><span>Active invitations cannot be redeemed while invitations are disabled</span><strong>Review</strong></Link>{/if}
            </div>
          {:else}
            <p class="dashboard-status"><Icon name="check"/> No administrative follow-up is needed.</p>
          {/if}
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>{isOwner ? "Recent activity" : "Recent accounts"}</h2></div></div>
          <div class="data-list compact-data-list">
            {#if isOwner}
              {#each events.slice(0, 5) as event (event.id)}
                {@const destination = activityUrl(event)}
                {#if destination}<Link href={destination} class="data-row"><div><strong>{activityLabel(event.action)}</strong><small>{event.actor_username} · {formatDate(event.created_at)}{event.target_label ? ` · ${event.target_label}` : ""}</small></div></Link>
                {:else}<article class="data-row"><div><strong>{activityLabel(event.action)}</strong><small>{event.actor_username} · {formatDate(event.created_at)}{event.target_label ? ` · ${event.target_label}` : ""}</small></div></article>{/if}
              {:else}<div class="empty"><p>No administrative activity yet.</p></div>{/each}
            {:else}
              {#each recentUsers as user (user.id)}
                <Link class="data-row" href={`/admin/users/${user.id}`}><div><strong>{user.username}</strong><small>Joined {formatDate(user.created_at)}</small></div><span class="badge">{user.role === "admin" ? "Administrator" : "User"}</span></Link>
              {:else}<div class="empty"><p>No accounts yet.</p></div>{/each}
            {/if}
          </div>
        </section>

        <section class="panel dashboard-section">
          <div class="section-heading"><div><h2>Quick actions</h2></div></div>
          <div class="dashboard-actions">
            {#if $appState.config.invitations_enabled}<button class="button primary" type="button" onclick={() => invitationDialog.open()}><Icon name="plus"/> Create invitation</button>{/if}
            <Link class="button" href="/admin/users"><Icon name="search"/> Find user</Link>
            <Link class="button" href="/admin/pastes"><Icon name="search"/> Search pastes</Link>
          </div>
        </section>
      {/if}
    </div>
  </div>
</section>
