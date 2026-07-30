<script lang="ts">
  import { onMount } from "svelte";
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import { formatByteSize, formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import { deferRouteReady } from "../router";
  import type { Page, Paste, User } from "../types";

  let { query }: { query: URLSearchParams } = $props();
  let page = $state<Page<Paste> | null>(null);
  let users = $state<User[]>([]);
  let ownerNames = $derived(new Map(users.map(user => [user.id, user.username])));
  const initialLoadReady = deferRouteReady();

  onMount(() => {
    const params = new URLSearchParams(query);
    params.set("page_size", "100");
    void Promise.all([
      requestApi<Page<Paste>>(`/admin/pastes?${params}`),
      requestApi<User[]>("/admin/users")
    ]).then(([result, loadedUsers]) => {
      page = result;
      users = loadedUsers;
    }).catch(error => showNotice(error instanceof Error ? error.message : "Unable to load pastes", "error"))
      .finally(initialLoadReady);
  });

  function filterUrl(key: string, value: string): string {
    const params = new URLSearchParams(query);
    params.set(key, value);
    params.delete("page");
    return `/admin/pastes?${params}`;
  }

  async function copy(paste: Paste): Promise<void> {
    await navigator.clipboard.writeText(`${location.origin}/pastes/${paste.id}`);
    showNotice("Link copied.");
  }

  async function remove(paste: Paste): Promise<void> {
    if (!confirm("Delete this paste permanently?")) return;
    try {
      await requestApi(`/pastes/${encodeURIComponent(paste.id)}`, { method: "DELETE" });
      if (page) page = { ...page, items: page.items.filter(item => item.id !== paste.id), total_items: page.total_items - 1 };
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }
</script>

<section>
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><Link class="button" href="/admin">Admin home</Link></div>
  <PasteFilters params={query} mode="admin" {ownerNames}/>
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
            <Link href={filterUrl(paste.content_kind === "text" ? "language" : "content_kind", paste.content_kind === "text" ? paste.language : paste.content_kind)}>{pasteFormatLabel(paste)}</Link>
            <Link href={filterUrl("visibility", paste.visibility)}>{paste.visibility}</Link>
            {#if paste.attachment_count}<Link href={filterUrl("has_attachments", "true")}>{paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}</Link>{/if}
            <span>{formatByteSize(paste.size_bytes)}</span>
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
    <Pagination {page}/>
  {:else}<p class="muted">Loading pastes…</p>{/if}
</section>
