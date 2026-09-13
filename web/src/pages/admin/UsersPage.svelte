<script lang="ts">
  import { listAdminUsers } from "../../api";
  import Icon from "../../components/Icon.svelte";
  import AdminNav from "../../components/AdminNav.svelte";
  import Link from "../../components/Link.svelte";
  import InvitationDialog from "../../components/InvitationDialog.svelte";
  import Pagination from "../../components/Pagination.svelte";
  import { formatByteSize, formatDate } from "../../format";
  import { appState } from "../../app/state";
  import { createPageLoader } from "../../app/pageLoader";
  import { navigate } from "../../navigation";
  import type { AdminUser, Page } from "../../types";

  let { query }: { query: URLSearchParams } = $props();
  let page = $state<Page<AdminUser> | null>(null);
  let search = $state("");
  let error = $state("");
  let loading = $state(false);
  let invitationDialog: InvitationDialog;
  const pageLoader = createPageLoader();

  $effect(() => {
    const source = new URLSearchParams(query);
    source.set("page_size", "25");
    loading = true;
    return pageLoader.load(() => listAdminUsers(source), {
      success: (value) => {
        page = value;
        search = query.get("search") ?? "";
        error = "";
      },
      failure: (reason) => {
        error = reason instanceof Error ? reason.message : "Unable to load users";
      },
      settled: () => {
        loading = false;
      }
    });
  });

  async function applySearch(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const params = new URLSearchParams(query);
    if (search.trim()) params.set("search", search.trim());
    else params.delete("search");
    params.delete("page");
    await navigate(`/admin/users?${params}`);
  }

  function setFilter(key: string, value: string): void {
    const params = new URLSearchParams(query);
    if (value) params.set(key, value);
    else params.delete(key);
    params.delete("page");
    void navigate(`/admin/users?${params}`);
  }
</script>

<InvitationDialog bind:this={invitationDialog} />

<section class="page-layout" aria-busy={loading}>
  <div class="page-heading">
    <div>
      <p class="eyebrow">Administration</p>
      <h1>Users</h1>
    </div>
    {#if $appState.config.invitations_enabled}<div class="page-heading-actions">
        <button class="button primary" type="button" onclick={() => invitationDialog.open()}
          ><Icon name="plus" /> Create invitation</button
        >
      </div>{/if}
  </div>
  <div class="section-layout">
    <AdminNav />
    <div class="section-content">
      <form class="list-filter-bar admin-user-filters" onsubmit={applySearch}>
        <label class="field list-filter-search"
          ><span>Search</span><input
            type="search"
            placeholder="Username"
            bind:value={search}
          /></label
        >
        <button class="button primary" type="submit"><Icon name="search" /> Search</button>
        <label class="field list-filter-select"
          ><span>Role</span><select
            value={query.get("role") ?? ""}
            onchange={(event) => setFilter("role", event.currentTarget.value)}
            ><option value="">Any role</option><option value="user">User</option><option
              value="admin">Administrator</option
            ><option value="owner">Owner</option></select
          ></label
        >
        <label class="field list-filter-select"
          ><span>Status</span><select
            value={query.get("status") ?? ""}
            onchange={(event) => setFilter("status", event.currentTarget.value)}
            ><option value="">Any status</option><option value="enabled">Enabled</option><option
              value="disabled">Disabled</option
            ></select
          ></label
        >
        <label class="field list-filter-select"
          ><span>Sort</span><select
            value={query.get("sort") ?? "username"}
            onchange={(event) => setFilter("sort", event.currentTarget.value)}
            ><option value="username">Username</option><option value="created">Newest</option
            ><option value="login">Last login</option><option value="pastes">Paste count</option
            ><option value="storage">Storage</option></select
          ></label
        >
        <label class="field list-filter-select"
          ><span>Direction</span><select
            value={query.get("direction") ??
              ((query.get("sort") ?? "username") === "username" ? "asc" : "desc")}
            onchange={(event) => setFilter("direction", event.currentTarget.value)}
            ><option value="asc">Ascending</option><option value="desc">Descending</option></select
          ></label
        >
      </form>
      {#if error}<section class="empty">
          <h2>Unable to load users</h2>
          <p>{error}</p>
        </section>
      {:else if page}<p class="result-count">{page.total_items} users</p>
        <div class="panel admin-user-table" role="table" aria-label="Users">
          <div class="admin-user-row admin-user-header" role="row">
            <span>User</span><span>Access</span><span>Activity</span><span>Usage</span><span></span>
          </div>
          {#each page.items as user (user.id)}
            <div class="admin-user-row" role="row">
              <div>
                <Link href={`/admin/users/${user.id}`}><strong>{user.username}</strong></Link><small
                  >Joined {formatDate(user.created_at)}</small
                >
              </div>
              <div class="badge-group">
                <span class="badge"
                  >{user.role === "owner"
                    ? "Owner"
                    : user.role === "admin"
                      ? "Administrator"
                      : "User"}</span
                ><span class:danger={!user.enabled} class="badge"
                  >{user.enabled ? "Enabled" : "Disabled"}</span
                >
              </div>
              <div>
                <span
                  >{user.last_login_at ? formatDate(user.last_login_at) : "Never logged in"}</span
                ><small
                  >{user.active_session_count} active {user.active_session_count === 1
                    ? "session"
                    : "sessions"}</small
                >
              </div>
              <div>
                <span>{user.paste_count} {user.paste_count === 1 ? "paste" : "pastes"}</span><small
                  >{formatByteSize(user.storage_bytes)} · {user.active_api_key_count} active keys</small
                >
              </div>
              <Link class="button" href={`/admin/users/${user.id}`}>Manage</Link>
            </div>
          {:else}<div class="empty"><p>No users match these filters.</p></div>{/each}
        </div>
        <Pagination {page} params={query} />{:else}<p class="muted">Loading users…</p>{/if}
    </div>
  </div>
</section>
