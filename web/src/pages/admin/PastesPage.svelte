<script lang="ts">
  import { deletePaste, listAdminPastes } from "../../api";
  import Icon from "../../components/Icon.svelte";
  import AdminNav from "../../components/AdminNav.svelte";
  import Link from "../../components/Link.svelte";
  import Pagination from "../../components/Pagination.svelte";
  import PasteFilters from "../../components/PasteFilters.svelte";
  import { formatByteSize, formatDate, pasteDisplayTitle, pasteFormatLabel } from "../../format";
  import { confirmAction } from "../../app/confirmations";
  import { showNotice } from "../../app/notices";
  import { cachedQuery, loadQuery } from "../../app/queryCache";
  import { holdNavigation } from "../../navigation";
  import { appState } from "../../app/state";
  import type { Page, Paste } from "../../types";

  let { query }: { query: URLSearchParams } = $props();
  function pastePath(requestedQuery: URLSearchParams): string {
    const params = new URLSearchParams(requestedQuery);
    if (params.has("search")) {
      params.set("q", params.get("search") ?? "");
      params.delete("search");
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
    query: URLSearchParams;
  } {
    const requestedQuery = new URLSearchParams(query);
    const cachedPage = cachedQuery<Page<Paste>>(pastePath(requestedQuery));
    const complete = Boolean(cachedPage);
    return {
      page: complete ? cachedPage ?? null : null,
      query: complete ? requestedQuery : new URLSearchParams()
    };
  }

  const initial = initialState();
  let page = $state<Page<Paste> | null>(initial.page);
  let appliedQuery = $state(initial.query);
  let loading = $state(false);
  let error = $state("");
  let ownerNames = $derived(new Map(page?.items.filter(paste => paste.owner_id && paste.owner_username).map(paste => [paste.owner_id!, paste.owner_username!]) ?? []));
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
    if (cachedPage) {
      page = cachedPage;
      appliedQuery = requestedQuery;
      error = "";
    }
    void loadQuery(requestedPastePath, () => listAdminPastes(new URLSearchParams(requestedPastePath.split("?")[1]))).then(result => {
      if (generation !== loadGeneration) return;
      page = result;
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
    if (!(await confirmAction({ title: "Delete paste?", message: "This paste and its attachments will be permanently deleted.", confirmLabel: "Delete paste", dangerous: true }))) return;
    try {
      await deletePaste(paste.id, paste._etag ?? "*");
      if (page) page = { ...page, items: page.items.filter(item => item.id !== paste.id), total_items: page.total_items - 1 };
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }
</script>

<section class="page-layout" aria-busy={loading}>
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div></div>
  <div class="section-layout"><AdminNav/><div class="section-content">
  <PasteFilters params={appliedQuery} mode="admin" {ownerNames}/>
  {#if page}
    <p class="result-count">{page.total_items} pastes</p>
    <div class="admin-paste-head" aria-hidden="true"><span>Paste</span><span>Owner</span><span>Details</span><span>Actions</span></div>
    <div class="admin-paste-list">
      {#each page.items as paste (paste.id)}
        <article class="admin-paste-row paste-row">
          <div class="paste-main"><Link class="paste-title" href={`/pastes/${paste.id}`}>{pasteDisplayTitle(paste)}</Link>
            <p>{paste.content.slice(0, 160).replace(/\s+/g, " ")}</p>
            <div class="paste-identity-meta"><code>{paste.id}</code><time datetime={new Date(paste.created_at * 1000).toISOString()}>{formatDate(paste.created_at)}</time></div></div>
          <div class="admin-paste-owner">
            {#if paste.owner_id === null}<span class="muted">No owner</span>
            {:else}<Link href={filterUrl("owner_id", String(paste.owner_id))}><strong>{paste.owner_username ?? `User #${paste.owner_id}`}</strong><small>User #{paste.owner_id}</small></Link>{/if}
          </div>
          <div class="paste-meta">
            <Link class="meta-badge" href={filterUrl(paste.format === "text" ? "language" : "format", paste.format === "text" ? paste.language : paste.format)}>{pasteFormatLabel(paste)}</Link>
            <Link class="meta-badge" href={filterUrl("visibility", paste.visibility)}>{paste.visibility}</Link>
            {#if paste.attachment_count}<Link class="meta-detail" href={filterUrl("has_attachments", "true")}>{paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}</Link>{/if}
            <span class="meta-detail">{formatByteSize(paste.size_bytes)}</span>
            <span class="meta-detail">{paste.read_count} view{paste.read_count === 1 ? "" : "s"}</span>
          </div>
          <div class="row-actions">
            <button class="icon-button" title="Copy link" aria-label="Copy link" type="button" onclick={() => copy(paste)}><Icon name="link-2"/></button>
            <Link class="icon-button" title="Edit" aria-label="Edit" href={`/pastes/${paste.id}/edit`}><Icon name="edit-3"/></Link>
            <button class="icon-button" title="Delete" aria-label="Delete" type="button" onclick={() => remove(paste)}><Icon name="trash-2"/></button>
          </div>
        </article>
      {:else}<div class="empty compact"><p>No pastes found.</p></div>{/each}
    </div>
    <Pagination {page} params={appliedQuery}/>
  {:else if error}<div class="empty compact"><p>{error}</p></div>
  {:else}<p class="muted">Loading pastes…</p>{/if}
  </div></div>
</section>
