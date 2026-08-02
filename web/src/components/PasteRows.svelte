<script lang="ts">
  import { requestApi } from "../api";
  import { formatByteSize, formatDate, pasteDisplayTitle, pasteFormatLabel } from "../format";
  import { showNotice } from "../notices";
  import type { Paste } from "../types";
  import type { PasteListView } from "../uiPreferences";
  import Icon from "./Icon.svelte";
  import Link from "./Link.svelte";

  let {
    items,
    manage = false,
    ownerNames,
    filterable = false,
    selectable = false,
    selected = $bindable(new Set<string>()),
    folderNames,
    view = "normal"
  }: {
    items: Paste[];
    manage?: boolean;
    ownerNames?: Map<number, string>;
    filterable?: boolean;
    selectable?: boolean;
    selected?: Set<string>;
    folderNames?: Map<number, string>;
    view?: PasteListView;
  } = $props();

  let visible = $state<Paste[]>([]);
  let rangeAnchor = $state<number | null>(null);
  $effect(() => {
    visible = items;
    rangeAnchor = null;
  });

  function filterUrl(key: string, value: string): string {
    const params = new URLSearchParams(location.search);
    params.set(key, value);
    params.delete("page");
    return `${location.pathname}?${params}`;
  }

  async function copyLink(paste: Paste): Promise<void> {
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
      visible = visible.filter(candidate => candidate.id !== paste.id);
      showNotice("Paste deleted.");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  function selectPaste(index: number, checked: boolean, extendRange: boolean): void {
    const next = new Set(selected);
    if (extendRange && rangeAnchor !== null) {
      const start = Math.min(rangeAnchor, index);
      const end = Math.max(rangeAnchor, index);
      for (const paste of visible.slice(start, end + 1)) {
        if (checked) next.add(paste.id); else next.delete(paste.id);
      }
    } else {
      const id = visible[index]?.id;
      if (id) {
        if (checked) next.add(id); else next.delete(id);
      }
    }
    if (!extendRange || rangeAnchor === null) rangeAnchor = index;
    selected = next;
  }
</script>

{#if visible.length === 0}
  <div class="empty compact"><p>No pastes found.</p></div>
{:else}
  {#if selectable}
    <span class="visually-hidden" id="paste-range-selection-help">
      Hold Shift while selecting to select a range.
    </span>
  {/if}
  <div class="paste-list" class:compact={view === "compact"}>
    {#each visible as paste, index (paste.id)}
      <article class="paste-row">
        <div class="paste-main">
          {#if selectable}<input type="checkbox" aria-label={`Select ${pasteDisplayTitle(paste)}`}
            aria-describedby="paste-range-selection-help" title="Shift-click to select a range"
            checked={selected.has(paste.id)}
            onclick={(event) => selectPaste(index, event.currentTarget.checked, event.shiftKey)}/>{/if}
          <div class="paste-main-content">
            <Link class="paste-title" href={`/pastes/${paste.id}`}>{pasteDisplayTitle(paste)}</Link>
            <p>{paste.content.slice(0, 160).replace(/\s+/g, " ")}</p>
            <div class="paste-row-footer">
              <div class="paste-meta">
                {#if ownerNames}
                  <span class="meta-detail">Owner: {paste.owner_id === null ? "No owner" : ownerNames.get(paste.owner_id) ?? `User #${paste.owner_id}`}</span>
                {/if}
                {#if filterable}
                  <Link class="meta-badge" href={filterUrl(
                    paste.content_kind === "text" ? "language" : "content_kind",
                    paste.content_kind === "text" ? paste.language : paste.content_kind
                  )}>{pasteFormatLabel(paste)}</Link>
                  <Link class="meta-badge" href={filterUrl("visibility", paste.visibility)}>{paste.visibility}</Link>
                  {#if paste.folder_id && folderNames}
                    <Link class="meta-detail" href={filterUrl("folder_id", String(paste.folder_id))}>
                      Folder: {folderNames.get(paste.folder_id) ?? "Unknown"}
                    </Link>
                  {/if}
                  {#if paste.attachment_count}
                    <Link class="meta-detail" href={filterUrl("has_attachments", "true")}>
                      {paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}
                    </Link>
                  {/if}
                {:else}
                  <span class="meta-badge">{pasteFormatLabel(paste)}</span>
                  <span class="meta-badge">{paste.visibility}</span>
                  {#if paste.attachment_count}
                    <span class="meta-detail">{paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}</span>
                  {/if}
                {/if}
                <span class="meta-detail">{formatByteSize(paste.size_bytes)}</span>
                <time class="meta-detail" datetime={new Date(paste.created_at * 1000).toISOString()}>{formatDate(paste.created_at)}</time>
              </div>
              <div class="row-actions">
                <button class="icon-button" type="button" title="Copy link" aria-label="Copy link"
                  onclick={() => copyLink(paste)}><Icon name="copy"/></button>
                {#if manage}
                  <Link class="icon-button" title="Edit" aria-label="Edit"
                    href={`/pastes/${paste.id}/edit`}><Icon name="edit-3"/></Link>
                  <button class="icon-button" type="button" title="Delete" aria-label="Delete"
                    onclick={() => remove(paste)}><Icon name="trash-2"/></button>
                {/if}
              </div>
            </div>
          </div>
        </div>
      </article>
    {/each}
  </div>
{/if}
