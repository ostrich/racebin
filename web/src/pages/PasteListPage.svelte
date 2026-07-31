<script lang="ts">
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import FolderNav from "../components/FolderNav.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { showNotice } from "../notices";
  import { deferRouteReady, navigate } from "../router";
  import type { FolderOverview, Page, Paste } from "../types";
  import { uiPreferences } from "../uiPreferences";

  let { mine, query }: { mine: boolean; query: URLSearchParams } = $props();
  let page = $state<Page<Paste> | null>(null);
  let error = $state("");
  let folders = $state<FolderOverview | null>(null);
  let appliedQuery = $state(new URLSearchParams());
  let loading = $state(false);
  let reloadToken = $state(0);
  let selected = $state(new Set<string>());
  let moveFolder = $state("");
  let currentFolderId = $derived(appliedQuery.get("folder_id") ? Number(appliedQuery.get("folder_id")) : null);
  let unfiled = $derived(appliedQuery.get("unfiled") === "true");
  let folderNames = $derived(new Map((folders?.items ?? []).map(folder => [folder.id, folder.name])));
  let currentFolderName = $derived(unfiled ? "Uncategorized"
    : currentFolderId ? folderNames.get(currentFolderId) ?? "Folder" : "My pastes");
  let loadGeneration = 0;
  let initialRouteReady: (() => void) | null = deferRouteReady();

  $effect(() => {
    reloadToken;
    const requestedQuery = new URLSearchParams(query);
    const generation = ++loadGeneration;
    const routeReady = initialRouteReady ?? deferRouteReady();
    initialRouteReady = null;
    loading = true;
    const params = new URLSearchParams(requestedQuery);
    params.set("page_size", "50");
    if (mine) params.set("mine", "true");
    else params.set("visibility", "public");
    void Promise.all([
      requestApi<Page<Paste>>(`/pastes?${params}`),
      mine ? requestApi<FolderOverview>("/folders") : Promise.resolve(null)
    ])
      .then(([result, loadedFolders]) => {
        if (generation !== loadGeneration) return;
        page = result;
        folders = loadedFolders;
        appliedQuery = requestedQuery;
        selected = new Set();
        error = "";
      })
      .catch(reason => {
        if (generation !== loadGeneration) return;
        const message = reason instanceof Error ? reason.message : "Unable to load pastes";
        if (!page) error = message;
        showNotice(message, "error");
      })
      .finally(() => {
        if (generation === loadGeneration) loading = false;
        routeReady();
      });
  });

  async function createFolder(): Promise<void> {
    const name = prompt("Folder name");
    if (!name?.trim()) return;
    try {
      const folder = await requestApi<{id:number}>("/folders", {
        method: "POST", body: JSON.stringify({ name })
      });
      await navigate(`/pastes?folder_id=${folder.id}`);
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to create folder", "error"); }
  }

  async function renameFolder(id: number, current: string): Promise<void> {
    const name = prompt("Folder name", current);
    if (!name?.trim() || name.trim() === current) return;
    try {
      await requestApi(`/folders/${id}`, { method: "PATCH", body: JSON.stringify({ name }) });
      if (folders) folders = { ...folders, items: folders.items.map(folder =>
        folder.id === id ? { ...folder, name: name.trim() } : folder) };
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to rename folder", "error"); }
  }

  async function deleteFolder(id: number, name: string): Promise<void> {
    if (!confirm(`Delete “${name}”? Its pastes will move to Uncategorized.`)) return;
    try {
      await requestApi(`/folders/${id}`, { method: "DELETE" });
      if (currentFolderId === id) await navigate("/pastes?unfiled=true");
      else reloadToken += 1;
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to delete folder", "error"); }
  }

  async function moveSelected(): Promise<void> {
    if (!selected.size) return;
    try {
      await requestApi("/pastes/folder", {
        method: "PATCH",
        body: JSON.stringify({
          paste_ids: [...selected],
          folder_id: moveFolder ? Number(moveFolder) : null
        })
      });
      selected = new Set();
      reloadToken += 1;
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to move pastes", "error");
    }
  }
</script>

<section class:paste-workspace={mine} aria-busy={loading}
  class:folder-sidebar-collapsed={mine && $uiPreferences.folderSidebarCollapsed}>
  {#if mine && folders}
    <FolderNav overview={folders} {currentFolderId} {unfiled}
      oncreate={createFolder} onrename={renameFolder} ondelete={deleteFolder}/>
  {/if}
  <div class="paste-workspace-main">
  <div class="page-heading">
    <div><p class="eyebrow">{mine ? "Workspace" : "Public"}</p><h1>{mine ? currentFolderName : "Explore"}</h1></div>
    {#if mine}<Link class="button primary" href={`/pastes/new${currentFolderId ? `?folder_id=${currentFolderId}` : ""}`}><Icon name="plus"/> New paste</Link>{/if}
  </div>
  <PasteFilters params={appliedQuery} mode={mine ? "mine" : "explore"}/>
  {#if page}
    <p class="result-count">{page.total_items} paste{page.total_items === 1 ? "" : "s"}</p>
    {#if mine && page.items.length}
      <div class="paste-bulk-actions">
        <label><input type="checkbox" checked={selected.size === page.items.length}
          onchange={(event) => { selected = event.currentTarget.checked
            ? new Set(page?.items.map(item => item.id)) : new Set(); }}/> Select all on page</label>
        <select bind:value={moveFolder} aria-label="Move selected to folder">
          <option value="">Uncategorized</option>
          {#each folders?.items ?? [] as folder}<option value={folder.id}>{folder.name}</option>{/each}
        </select>
        <button class="button" type="button" disabled={!selected.size}
          onclick={() => void moveSelected()}>Move {selected.size || ""}</button>
      </div>
    {/if}
    <PasteRows items={page.items} manage={mine} filterable selectable={mine}
      bind:selected {folderNames}/>
    <Pagination {page} params={appliedQuery}/>
  {:else if error}
    <div class="empty compact"><p>{error}</p></div>
  {:else}
    <p class="muted">Loading pastes…</p>
  {/if}
  </div>
</section>
