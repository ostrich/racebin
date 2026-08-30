<script lang="ts">
  import { listInvitations, revokeInvitation, type Invitation } from "../../api";
  import AdminNav from "../../components/AdminNav.svelte";
  import Icon from "../../components/Icon.svelte";
  import InvitationDialog from "../../components/InvitationDialog.svelte";
  import Link from "../../components/Link.svelte";
  import Pagination from "../../components/Pagination.svelte";
  import { confirmAction } from "../../app/confirmations";
  import { formatDate } from "../../format";
  import { holdNavigation, navigate } from "../../navigation";
  import { showNotice } from "../../app/notices";
  import { appState } from "../../app/state";
  import type { Page } from "../../types";

  let { query }: { query: URLSearchParams } = $props();
  let page = $state<Page<Invitation> | null>(null);
  let activeCount = $state(0);
  let error = $state("");
  let loading = $state(false);
  let search = $state("");
  let invitationDialog: InvitationDialog;
  let generation = 0;
  let initialRouteReady: (() => void) | null = holdNavigation();
  let view = $derived(query.get("view") === "history" ? "history" : "active");
  const statusLabel = (status: Invitation["status"]) =>
    status.charAt(0).toUpperCase() + status.slice(1);

  function apiQuery(source: URLSearchParams, pageSize = 25): URLSearchParams {
    const result = new URLSearchParams(source);
    result.set("view", result.get("view") === "history" ? "history" : "active");
    result.set("page_size", String(pageSize));
    return result;
  }

  async function load(source = query): Promise<void> {
    const current = ++generation;
    loading = true;
    const selected = apiQuery(source);
    const active = new URLSearchParams({ view: "active", page_size: "1" });
    try {
      const [result, activePage] = await Promise.all([
        listInvitations(selected),
        selected.get("view") === "active" ? Promise.resolve(null) : listInvitations(active)
      ]);
      if (current !== generation) return;
      page = result;
      activeCount = activePage?.total_items ?? result.total_items;
      search = source.get("search") ?? "";
      error = "";
    } catch (reason) {
      if (current !== generation) return;
      error = reason instanceof Error ? reason.message : "Unable to load invitations";
    } finally {
      if (current === generation) loading = false;
    }
  }

  $effect(() => {
    const source = new URLSearchParams(query);
    const ready = initialRouteReady;
    initialRouteReady = null;
    void load(source).finally(() => ready?.());
  });

  function tabUrl(nextView: "active" | "history"): string {
    const params = new URLSearchParams();
    if (nextView === "history") params.set("view", "history");
    return `/admin/invitations${params.size ? `?${params}` : ""}`;
  }

  async function applyFilters(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const params = new URLSearchParams(query);
    if (search.trim()) params.set("search", search.trim());
    else params.delete("search");
    params.delete("page");
    await navigate(`/admin/invitations?${params}`);
  }

  async function copy(invitation: Invitation): Promise<void> {
    if (!invitation.url) return;
    await navigator.clipboard.writeText(new URL(invitation.url, location.origin).href);
    showNotice("Invitation link copied.");
  }

  async function revoke(invitation: Invitation): Promise<void> {
    if (
      !(await confirmAction({
        title: "Revoke invitation?",
        message: `${invitation.comment ? `“${invitation.comment}”` : "This invitation"} will no longer be usable.`,
        confirmLabel: "Revoke invitation",
        dangerous: true
      }))
    )
      return;
    await revokeInvitation(invitation.id);
    await load();
    showNotice("Invitation revoked.");
  }
</script>

<InvitationDialog bind:this={invitationDialog} oncreated={() => load()} />

<section class="page-layout" aria-busy={loading}>
  <div class="page-heading">
    <div>
      <p class="eyebrow">Administration</p>
      <h1>Invitations</h1>
    </div>
    {#if $appState.config.invitations_enabled}<div class="page-heading-actions">
        <button class="button primary" onclick={() => invitationDialog.open()}
          ><Icon name="plus" /> Create invitation</button
        >
      </div>{/if}
  </div>
  <div class="section-layout">
    <AdminNav />
    <div class="section-content invitation-management">
      {#if !$appState.config.invitations_enabled}
        <div class="notice">
          Invitation redemption is disabled in site settings. Existing invitations can still be
          reviewed or revoked.
        </div>
      {/if}
      <nav class="section-tabs" aria-label="Invitation records">
        <Link href={tabUrl("active")} aria-current={view === "active" ? "page" : undefined}
          >Active <span>{activeCount}</span></Link
        >
        <Link href={tabUrl("history")} aria-current={view === "history" ? "page" : undefined}
          >History</Link
        >
      </nav>
      {#if view === "history"}
        <form class="list-filter-bar invitation-filters" onsubmit={applyFilters}>
          <label class="field list-filter-search"
            ><span>Search</span><input
              type="search"
              bind:value={search}
              placeholder="Note, creator, recipient, or token"
            /></label
          >
          <button class="button primary" type="submit"><Icon name="search" /> Search</button>
          <label class="field list-filter-select"
            ><span>Status</span><select
              name="status"
              value={query.get("status") ?? ""}
              onchange={(event) => {
                const params = new URLSearchParams(query);
                const value = event.currentTarget.value;
                if (value) params.set("status", value);
                else params.delete("status");
                params.delete("page");
                void navigate(`/admin/invitations?${params}`);
              }}
              ><option value="">All statuses</option><option value="redeemed">Redeemed</option
              ><option value="revoked">Revoked</option><option value="expired">Expired</option
              ></select
            ></label
          >
        </form>
      {/if}
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else if page}
        <div class="panel data-list invitation-list">
          {#each page.items as invitation (invitation.id)}
            <article class="data-row invitation-row">
              <div class="invitation-identity">
                <strong>{invitation.comment || "Invitation"}</strong>
                <small
                  >Created by {invitation.created_by_username} · {formatDate(
                    invitation.created_at
                  )}</small
                >
              </div>
              <div class="invitation-lifecycle">
                <span
                  class:danger={invitation.status === "expired" || invitation.status === "revoked"}
                  class="badge">{statusLabel(invitation.status)}</span
                >
                <small
                  >{invitation.redeemed_by_username
                    ? `Redeemed by ${invitation.redeemed_by_username}${invitation.redeemed_at ? ` · ${formatDate(invitation.redeemed_at)}` : ""}`
                    : `Expires ${formatDate(invitation.expires_at)}`}</small
                >
                <code>{invitation.token_prefix}…</code>
              </div>
              <div class="row-actions">
                <button
                  class="icon-button"
                  title="Edit private note"
                  aria-label="Edit private note"
                  onclick={() => invitationDialog.edit(invitation)}><Icon name="edit-3" /></button
                >
                {#if invitation.url}<button
                    class="icon-button"
                    title="Copy invitation link"
                    aria-label="Copy invitation link"
                    onclick={() => copy(invitation)}><Icon name="link-2" /></button
                  >{/if}
                {#if invitation.status === "active"}<button
                    class="button danger"
                    onclick={() => revoke(invitation)}>Revoke</button
                  >{/if}
              </div>
            </article>
          {:else}
            <div class="empty">
              <p>
                {view === "active"
                  ? "No active invitations."
                  : "No invitation history matches these filters."}
              </p>
            </div>
          {/each}
        </div>
        <Pagination {page} params={query} />
      {/if}
    </div>
  </div>
</section>
