<script lang="ts">
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import { formatByteSize, formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import { cachedQuery, loadQuery } from "../queryCache";
  import { holdNavigation } from "../navigation";
  import { appState } from "../state";
  import type { Page, Paste, User } from "../types";

  let { query }: { query: URLSearchParams } = $props();
  function pastePath(requestedQuery: URLSearchParams): string {
    const params = new URLSearchParams(requestedQuery);
    if (params.has("search")) {
      params.set("q", params.get("search") ?? "");
      params.delete("search");
    }
    if (params.has("content_kind")) {
      params.set("format", params.get("content_kind") ?? "");
      params.delete("content_kind");
    }
    for (const key of ["created_after", "created_before"]) {
      const value = params.get(key);
      if (value && Number.isFinite(Number(value))) {
        params.set(key, new Date(Number(value) * 1000).toISOString());
      }
    }
    params.set("page_size", String($appState.config.max_page_size));
    return `/admin/pastes?${params}`;
  }

  function initialState(): {
    page: Page<Paste> | null;
    users: User[];
    query: URLSearchParams;
  } {
    const requestedQuery = new URLSearchParams(query);
    const cachedPage = cachedQuery<Page<Paste>>(pastePath(requestedQuery));
    const cachedUsers = cachedQuery<User[]>("/admin/users");
    const complete = Boolean(cachedPage && cachedUsers);
    return {
      page: complete ? cachedPage ?? null : null,
      users: complete ? cachedUsers ?? [] : [],
      query: complete ? requestedQuery : new URLSearchParams()
    };
  }

  const initial = initialState();
  let page = $state<Page<Paste> | null>(initial.page);
  let users = $state<User[]>(initial.users);
  let appliedQuery = $state(initial.query);
  let loading = $state(false);
  let error = $state("");
  let ownerNames = $derived(new Map(users.map(user => [user.id, user.username])));
  let loadGeneration = 0;
  let initialRouteReady: (() => void) | null = holdNavigation();

  $effect(() => {
    const requestedQuery = new URLSearchParams(query);
    const generation = ++loadGeneration;
    const routeReady = initialRouteReady ?? holdNavigation();
    initialRouteReady = null;
    loading = true;
    const requestedPastePath = pastePath(requestedQuery);
    const cachedPage = cachedQuery<Page<Paste>>(requestedPastePath);
    const cachedUsers = cachedQuery<User[]>("/admin/users");
    if (cachedPage && cachedUsers) {
      page = cachedPage;
      users = cachedUsers;
      appliedQuery = requestedQuery;
      error = "";
    }
    void Promise.all([
      loadQuery(requestedPastePath, () => requestApi<Page<Paste>>(requestedPastePath)),
      loadQuery("/admin/users", () => requestApi<User[]>("/admin/users"))
    ]).then(([result, loadedUsers]) => {
      if (generation !== loadGeneration) return;
      page = result;
      users = loadedUsers;
      appliedQuery = requestedQuery;
      error = "";
    }).catch(reason => {
      if (generation !== loadGeneration) return;
      const message = reason instanceof Error ? reason.message : "Unable to load pastes";
      if (!page) error = message;
      showNotice(message, "error");
    }).finally(() => {
      if (generation === loadGeneration) loading = false;
      routeReady();
    });
  });

  function filterUrl(key: string, value: string): string {
    const params = new URLSearchParams(appliedQuery);
    params.set(key, value);
    params.delete("page");
    return `/admin/pastes?${params}`;
  }

  async function copy(paste: Paste): Promise<void> {
    await navigator.clipboard.writeText(new URL(paste.url ?? `/pastes/${paste.id}`, location.origin).href);
    showNotice("Link copied.");
  }

  async function remove(paste: Paste): Promise<void> {
    if (!confirm("Delete this paste permanently?")) return;
    try {
      await requestApi(`/pastes/${encodeURIComponent(paste.id)}`, {
        method: "DELETE",
        headers: { "If-Match": paste._etag ?? "*" }
      });
      if (page) page = { ...page, items: page.items.filter(item => item.id !== paste.id), total_items: page.total_items - 1 };
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }
</script>

<section aria-busy={loading}>
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><Link class="button" href="/admin">Admin home</Link></div>
  <PasteFilters params={appliedQuery} mode="admin" {ownerNames}/>
  {#if page}
    <p class="result-count">{page.total_items} pastes</p>
    <div class="admin-paste-head" aria-hidden="true"><span>Paste</span><span>Owner</span><span>Metadata</span><span>Created</span><span>Actions</span></div>
    <div class="admin-paste-list">
      {#each page.items as paste (paste.id)}
        <article class="admin-paste-row paste-row">
          <div class="paste-main"><Link class="paste-title" href={`/pastes/${paste.id}`}>{pasteDisplayTitle(paste)}</Link>
            <p>{paste.content.slice(0, 160).replace(/\s+/g, " ")}</p><code>{paste.id}</code></div>
          <div class="admin-paste-owner">
            {#if paste.owner_id === null}<span class="muted">No owner</span>
            {:else}<Link href={filterUrl("owner_id", String(paste.owner_id))}><strong>{ownerNames.get(paste.owner_id) ?? `User #${paste.owner_id}`}</strong><small>User #{paste.owner_id}</small></Link>{/if}
          </div>
          <div class="paste-meta">
            <Link class="meta-badge" href={filterUrl(paste.content_kind === "text" ? "language" : "content_kind", paste.content_kind === "text" ? paste.language : paste.content_kind)}>{pasteFormatLabel(paste)}</Link>
            <Link class="meta-badge" href={filterUrl("visibility", paste.visibility)}>{paste.visibility}</Link>
            {#if paste.attachment_count}<Link class="meta-detail" href={filterUrl("has_attachments", "true")}>{paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}</Link>{/if}
            <span class="meta-detail">{formatByteSize(paste.size_bytes)}</span>
          </div>
          <time datetime={new Date(paste.created_at * 1000).toISOString()}>{formatDate(paste.created_at)}</time>
          <div class="row-actions">
            <button class="icon-button" title="Copy link" aria-label="Copy link" type="button" onclick={() => copy(paste)}><Icon name="copy"/></button>
            <Link class="icon-button" title="Edit" aria-label="Edit" href={`/pastes/${paste.id}/edit`}><Icon name="edit-3"/></Link>
            <button class="icon-button" title="Delete" aria-label="Delete" type="button" onclick={() => remove(paste)}><Icon name="trash-2"/></button>
          </div>
        </article>
      {:else}<div class="empty compact"><p>No pastes found.</p></div>{/each}
    </div>
    <Pagination {page} params={appliedQuery}/>
  {:else if error}<div class="empty compact"><p>{error}</p></div>
  {:else}<p class="muted">Loading pastes…</p>{/if}
</section>
