<script lang="ts">
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import FolderNav from "../components/FolderNav.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { showNotice } from "../notices";
  import { cachedQuery, loadQuery } from "../queryCache";
  import { holdNavigation, navigate } from "../navigation";
  import type { FolderOverview, Page, Paste, PasteRevisionResponse } from "../types";
  import { setPasteListView, uiPreferences } from "../uiPreferences";

  let { mine, query }: { mine: boolean; query: URLSearchParams } = $props();
  function requestPaths(requestedQuery: URLSearchParams): { paste: string; folders: string | null } {
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
    params.set("page_size", "50");
    if (mine) params.set("owner", "me");
    else params.set("visibility", "public");
    return { paste: `/pastes?${params}`, folders: mine ? "/folders" : null };
  }

  function initialState(): {
    page: Page<Paste> | null;
    folders: FolderOverview | null;
    query: URLSearchParams;
  } {
    const requestedQuery = new URLSearchParams(query);
    const paths = requestPaths(requestedQuery);
    const cachedPage = cachedQuery<Page<Paste>>(paths.paste);
    const cachedFolders = paths.folders
      ? cachedQuery<FolderOverview>(paths.folders)
      : null;
    const complete = Boolean(cachedPage && (!mine || cachedFolders));
    return {
      page: complete ? cachedPage ?? null : null,
      folders: complete ? cachedFolders ?? null : null,
      query: complete ? requestedQuery : new URLSearchParams()
    };
  }

  const initial = initialState();
  let page = $state<Page<Paste> | null>(initial.page);
  let error = $state("");
  let folders = $state<FolderOverview | null>(initial.folders);
  let appliedQuery = $state(initial.query);
  let loading = $state(false);
  let reloadToken = $state(0);
  let selected = $state(new Set<string>());
  let selectAllCheckbox = $state<HTMLInputElement>();
  let moveFolder = $state("");
  let currentFolderId = $derived(appliedQuery.get("folder_id") ? Number(appliedQuery.get("folder_id")) : null);
  let unfiled = $derived(appliedQuery.get("unfiled") === "true");
  let folderNames = $derived(new Map((folders?.items ?? []).map(folder => [folder.id, folder.name])));
  let currentFolderName = $derived(unfiled ? "Uncategorized"
    : currentFolderId ? folderNames.get(currentFolderId) ?? "Folder" : "My pastes");
  let loadGeneration = 0;
  let initialRouteReady: (() => void) | null = holdNavigation();

  $effect(() => {
    if (!selectAllCheckbox || !page) return;
    selectAllCheckbox.indeterminate = selected.size > 0 && selected.size < page.items.length;
  });

  $effect(() => {
    reloadToken;
    const requestedQuery = new URLSearchParams(query);
    const generation = ++loadGeneration;
    const routeReady = initialRouteReady ?? holdNavigation();
    initialRouteReady = null;
    loading = true;
    const paths = requestPaths(requestedQuery);
    const cachedPage = cachedQuery<Page<Paste>>(paths.paste);
    const cachedFolders = paths.folders
      ? cachedQuery<FolderOverview>(paths.folders)
      : null;
    if (cachedPage && (!mine || cachedFolders)) {
      page = cachedPage;
      folders = cachedFolders ?? null;
      appliedQuery = requestedQuery;
      selected = new Set();
      error = "";
    }
    void Promise.all([
      loadQuery(paths.paste, () => requestApi<Page<Paste>>(paths.paste)),
      paths.folders
        ? loadQuery(paths.folders, () => requestApi<FolderOverview>(paths.folders!))
        : Promise.resolve(null)
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
      await requestApi<PasteRevisionResponse>(`/folders/${id}`, { method: "DELETE" });
      if (currentFolderId === id) await navigate("/pastes?unfiled=true");
      else reloadToken += 1;
    } catch (reason) { showNotice(reason instanceof Error ? reason.message : "Unable to delete folder", "error"); }
  }

  async function moveSelected(): Promise<void> {
    if (!selected.size) return;
    try {
      await requestApi<PasteRevisionResponse>("/pastes", {
        method: "PATCH",
        body: JSON.stringify({
          ids: [...selected],
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
    {#if mine && page.items.length}
      <div class="paste-selection-bar">
        <div class="paste-view-controls">
          <div class="paste-view-switch" role="group" aria-label="Paste view">
            <button type="button" aria-pressed={$uiPreferences.pasteListView === "normal"}
              onclick={() => setPasteListView("normal")}>Normal</button>
            <button type="button" aria-pressed={$uiPreferences.pasteListView === "compact"}
              onclick={() => setPasteListView("compact")}>Compact</button>
          </div>
          <span class="result-count">{page.total_items} paste{page.total_items === 1 ? "" : "s"}</span>
        </div>
        <div class="paste-selection-controls">
          <div class="paste-bulk-actions">
            <select bind:value={moveFolder} aria-label="Move selected to folder">
              <option value="">Uncategorized</option>
              {#each folders?.items ?? [] as folder}<option value={folder.id}>{folder.name}</option>{/each}
            </select>
            <button class="button move-selected-button" type="button" disabled={!selected.size}
              onclick={() => void moveSelected()}>Move {selected.size || ""}</button>
          </div>
          <label class="select-all-pastes"><input bind:this={selectAllCheckbox} type="checkbox"
            checked={selected.size === page.items.length}
            onchange={(event) => { selected = event.currentTarget.checked
              ? new Set(page?.items.map(item => item.id)) : new Set(); }}/> Select all on page</label>
        </div>
      </div>
    {:else}
      <p class="result-count">{page.total_items} paste{page.total_items === 1 ? "" : "s"}</p>
    {/if}
    <PasteRows items={page.items} manage={mine} filterable selectable={mine}
      view={mine ? $uiPreferences.pasteListView : "normal"}
      bind:selected {folderNames}/>
    <Pagination {page} params={appliedQuery}/>
  {:else if error}
    <div class="empty compact"><p>{error}</p></div>
  {:else}
    <p class="muted">Loading pastes…</p>
  {/if}
  </div>
</section>
