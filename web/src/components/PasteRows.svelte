<script lang="ts">
  import { deletePaste } from "../api";
  import { pasteDisplayTitle } from "../format";
  import { confirmAction } from "../app/confirmations";
  import { showNotice } from "../app/notices";
  import type { Paste } from "../types";
  import type { PasteListView } from "../app/uiPreferences";
  import Link from "./Link.svelte";
  import PasteMetadata from "./PasteMetadata.svelte";
  import PasteRowActions from "./PasteRowActions.svelte";

  let {
    items,
    manage = false,
    ownerNames,
    filterable = false,
    selectable = false,
    selected = $bindable(new Set<string>()),
    folderNames,
    view = "normal",
    context = "public",
    totalItems,
    onremoved
  }: {
    items: Paste[];
    manage?: boolean;
    ownerNames?: Map<number, string>;
    filterable?: boolean;
    selectable?: boolean;
    selected?: Set<string>;
    folderNames?: Map<number, string>;
    view?: PasteListView;
    context?: "public" | "workspace" | "admin";
    totalItems?: number;
    onremoved?: (paste: Paste) => void;
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
    if (!(await confirmAction({ title: "Delete paste?", message: "This paste and its attachments will be permanently deleted.", confirmLabel: "Delete paste", dangerous: true }))) return;
    try {
      await deletePaste(paste.id, paste._etag ?? "*");
      visible = visible.filter(candidate => candidate.id !== paste.id);
      onremoved?.(paste);
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
  <div class:admin-paste-table={context === "admin"}>
    {#if context === "admin"}
      <div class="admin-paste-head" aria-hidden="true">
        <span>Paste{#if totalItems !== undefined}<small>{totalItems} total</small>{/if}</span>
        <span>Owner</span><span>Details</span><span>Actions</span>
      </div>
    {/if}
    <div class="paste-list" class:compact={view === "compact"}
      class:mobile-stack={context !== "admin" && view !== "compact"}
      class:admin-paste-list={context === "admin"}>
    {#each visible as paste, index (paste.id)}
      <article class="paste-row paste-list-row" class:selectable class:admin-paste-row={context === "admin"}>
        {#if context === "admin"}
          <div class="paste-row-identity">
            <Link class="paste-title" href={`/pastes/${paste.id}`}>{pasteDisplayTitle(paste)}</Link>
            <p>{paste.content.slice(0, 160).replace(/\s+/g, " ")}</p>
            <code class="paste-id">{paste.id}</code>
          </div>
          <div class="admin-paste-owner">
            {#if paste.owner_id === null}
              <span class="muted">No owner</span>
            {:else}
              <Link href={filterUrl("owner_id", String(paste.owner_id))}>
                <strong>{paste.owner_username ?? ownerNames?.get(paste.owner_id) ?? `User #${paste.owner_id}`}</strong>
                <small>User #{paste.owner_id}</small>
              </Link>
            {/if}
          </div>
          <PasteMetadata {paste} {filterable} {filterUrl} {folderNames}/>
          <PasteRowActions {paste} {manage} oncopy={copyLink} onremove={remove}/>
        {:else}
          {#if selectable}<input class="paste-selector" type="checkbox" aria-label={`Select ${pasteDisplayTitle(paste)}`}
            aria-describedby="paste-range-selection-help" title="Shift-click to select a range"
            checked={selected.has(paste.id)}
            onclick={(event) => selectPaste(index, event.currentTarget.checked, event.shiftKey)}/>{/if}
          <div class="paste-row-identity">
            <Link class="paste-title" href={`/pastes/${paste.id}`}>{pasteDisplayTitle(paste)}</Link>
            <p>{paste.content.slice(0, 160).replace(/\s+/g, " ")}</p>
          </div>
          <PasteMetadata {paste} {filterable} {filterUrl} {folderNames}/>
          <PasteRowActions {paste} {manage} oncopy={copyLink} onremove={remove}/>
        {/if}
      </article>
    {/each}
    </div>
  </div>
{/if}
