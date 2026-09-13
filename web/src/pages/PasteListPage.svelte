<script lang="ts">
  import {
    createFolder as createFolderRequest,
    deleteFolder as deleteFolderRequest,
    listFolders,
    listPastes,
    movePastes,
    renameFolder as renameFolderRequest
  } from "../api";
  import TextInputDialog from "../components/TextInputDialog.svelte";
  import FolderPicker from "../components/FolderPicker.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import Pagination from "../components/Pagination.svelte";
  import PasteFilters from "../components/PasteFilters.svelte";
  import PasteRows from "../components/PasteRows.svelte";
  import { confirmAction } from "../app/confirmations";
  import { showNotice } from "../app/notices";
  import { createPageLoader } from "../app/pageLoader";
  import { cachedQuery, loadQuery } from "../app/queryCache";
  import { navigate } from "../navigation";
  import type { FolderOverview, Page, Paste, PasteRevisionResponse } from "../types";
  import { setPasteListView, uiPreferences } from "../app/uiPreferences";

  let { mine, query }: { mine: boolean; query: URLSearchParams } = $props();
  function requestPaths(requestedQuery: URLSearchParams): {
    paste: string;
    folders: string | null;
  } {
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
    const cachedFolders = paths.folders ? cachedQuery<FolderOverview>(paths.folders) : null;
    const complete = Boolean(cachedPage && (!mine || cachedFolders));
    return {
      page: complete ? (cachedPage ?? null) : null,
      folders: complete ? (cachedFolders ?? null) : null,
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
  let folderNameDialog: TextInputDialog;
  let currentFolderId = $derived(
    appliedQuery.get("folder_id") ? Number(appliedQuery.get("folder_id")) : null
  );
  let unfiled = $derived(appliedQuery.get("unfiled") === "true");
  let folderNames = $derived(
    new Map((folders?.items ?? []).map((folder) => [folder.id, folder.name]))
  );
  let currentFolderName = $derived(
    unfiled
      ? "Uncategorized"
      : currentFolderId
        ? (folderNames.get(currentFolderId) ?? "Folder")
        : "My pastes"
  );
  const pageLoader = createPageLoader();

  $effect(() => {
    if (!selectAllCheckbox || !page) return;
    selectAllCheckbox.indeterminate = selected.size > 0 && selected.size < page.items.length;
  });

  $effect(() => {
    reloadToken;
    const requestedQuery = new URLSearchParams(query);
    loading = true;
    const paths = requestPaths(requestedQuery);
    const cachedPage = cachedQuery<Page<Paste>>(paths.paste);
    const cachedFolders = paths.folders ? cachedQuery<FolderOverview>(paths.folders) : null;
    if (cachedPage && (!mine || cachedFolders)) {
      page = cachedPage;
      folders = cachedFolders ?? null;
      appliedQuery = requestedQuery;
      selected = new Set();
      error = "";
    }
    return pageLoader.load(
      () =>
        Promise.all([
          loadQuery(paths.paste, () => listPastes(new URLSearchParams(paths.paste.split("?")[1]))),
          paths.folders ? loadQuery(paths.folders, () => listFolders()) : Promise.resolve(null)
        ]),
      {
        success: ([result, loadedFolders]) => {
          page = result;
          folders = loadedFolders;
          appliedQuery = requestedQuery;
          selected = new Set();
          error = "";
        },
        failure: (reason) => {
          const message = reason instanceof Error ? reason.message : "Unable to load pastes";
          if (!page) error = message;
          showNotice(message, "error");
        },
        settled: () => {
          loading = false;
        }
      }
    );
  });

  async function createFolder(): Promise<void> {
    const name = await folderNameDialog.ask({
      title: "Create folder",
      label: "Folder name",
      maximumLength: 64,
      submitLabel: "Create folder"
    });
    if (!name) return;
    try {
      const folder = await createFolderRequest(name);
      await navigate(`/pastes?folder_id=${folder.id}`);
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to create folder", "error");
    }
  }

  async function renameFolder(id: number, current: string): Promise<void> {
    const name = await folderNameDialog.ask({
      title: "Rename folder",
      label: "Folder name",
      value: current,
      maximumLength: 64,
      submitLabel: "Rename"
    });
    if (!name || name === current) return;
    try {
      await renameFolderRequest(id, name);
      if (folders)
        folders = {
          ...folders,
          items: folders.items.map((folder) => (folder.id === id ? { ...folder, name } : folder))
        };
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to rename folder", "error");
    }
  }

  async function deleteFolder(id: number, name: string): Promise<void> {
    if (
      !(await confirmAction({
        title: `Delete “${name}”?`,
        message: "The folder will be deleted and its pastes will move to Uncategorized.",
        confirmLabel: "Delete folder",
        dangerous: true
      }))
    )
      return;
    try {
      await deleteFolderRequest(id);
      if (currentFolderId === id) await navigate("/pastes?unfiled=true");
      else reloadToken += 1;
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete folder", "error");
    }
  }

  function folderUrl(id?: number, uncategorized = false): string {
    const params = new URLSearchParams(appliedQuery);
    params.delete("page");
    params.delete("folder_id");
    params.delete("unfiled");
    if (id) params.set("folder_id", String(id));
    if (uncategorized) params.set("unfiled", "true");
    return `/pastes${params.size ? `?${params}` : ""}`;
  }

  function browseFolder(folderId: number | null, browseUnfiled: boolean): void {
    void navigate(folderId ? folderUrl(folderId) : folderUrl(undefined, browseUnfiled));
  }

  async function moveSelected(folderId: number | null): Promise<void> {
    if (!selected.size) return;
    try {
      await movePastes({
        ids: [...selected],
        folder_id: folderId
      });
      selected = new Set();
      reloadToken += 1;
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to move pastes", "error");
    }
  }

  function pasteRemoved(paste: Paste): void {
    if (!page) return;
    const totalItems = Math.max(0, page.total_items - 1);
    const totalPages = Math.max(1, Math.ceil(totalItems / page.page_size));
    page = {
      ...page,
      items: page.items.filter((candidate) => candidate.id !== paste.id),
      total_items: totalItems,
      total_pages: totalPages
    };
    const nextSelected = new Set(selected);
    nextSelected.delete(paste.id);
    selected = nextSelected;
    if (page.page > totalPages) {
      const params = new URLSearchParams(appliedQuery);
      if (totalPages > 1) params.set("page", String(totalPages));
      else params.delete("page");
      void navigate(`/pastes${params.size ? `?${params}` : ""}`);
    }
  }
</script>

<TextInputDialog bind:this={folderNameDialog} />

<section class:paste-workspace={mine} aria-busy={loading}>
  <div class="paste-workspace-main">
    <div class="page-layout paste-list-intro">
      <div class="page-heading">
        <div>
          <p class="eyebrow">{mine ? "Workspace" : "Public"}</p>
          <h1>{mine ? currentFolderName : "Explore"}</h1>
        </div>
        {#if mine}<div class="page-heading-actions">
            <Link
              class="button primary"
              href={`/pastes/new${currentFolderId ? `?folder_id=${currentFolderId}` : ""}`}
              ><Icon name="plus" /> New paste</Link
            >
          </div>{/if}
      </div>
      <PasteFilters params={appliedQuery} mode={mine ? "mine" : "explore"} />
    </div>
    {#if page}
      {#if mine && folders}
        <div class="paste-selection-bar">
          <div class="paste-folder-controls">
            <FolderPicker
              overview={folders}
              mode="browse"
              label={currentFolderName}
              {currentFolderId}
              {unfiled}
              onselect={browseFolder}
              oncreate={createFolder}
              onrename={renameFolder}
              ondelete={deleteFolder}
            />
            <FolderPicker
              overview={folders}
              mode="move"
              label={selected.size ? `Move ${selected.size}` : "Move"}
              disabled={!selected.size}
              onselect={(folderId) => {
                void moveSelected(folderId);
              }}
            />
          </div>
          <div class="paste-view-switch segmented-control" role="group" aria-label="Paste view">
            <button
              type="button"
              aria-pressed={$uiPreferences.pasteListView === "normal"}
              onclick={() => setPasteListView("normal")}>Normal</button
            >
            <button
              type="button"
              aria-pressed={$uiPreferences.pasteListView === "compact"}
              onclick={() => setPasteListView("compact")}>Compact</button
            >
          </div>
          <label class="select-all-pastes"
            ><input
              bind:this={selectAllCheckbox}
              type="checkbox"
              disabled={!page.items.length}
              checked={page.items.length > 0 && selected.size === page.items.length}
              onchange={(event) => {
                selected = event.currentTarget.checked
                  ? new Set(page?.items.map((item) => item.id))
                  : new Set();
              }}
            /> Select all on page</label
          >
          <span class="result-count"
            >{page.total_items} paste{page.total_items === 1 ? "" : "s"}</span
          >
        </div>
      {:else}
        <p class="result-count">{page.total_items} paste{page.total_items === 1 ? "" : "s"}</p>
      {/if}
      <PasteRows
        items={page.items}
        manage={mine}
        filterable
        selectable={mine}
        context={mine ? "workspace" : "public"}
        view={mine ? $uiPreferences.pasteListView : "normal"}
        bind:selected
        folderNames={mine ? folderNames : undefined}
        onremoved={mine ? pasteRemoved : undefined}
      />
      <Pagination {page} params={appliedQuery} />
    {:else if error}
      <div class="empty compact"><p>{error}</p></div>
    {:else}
      <p class="muted">Loading pastes…</p>
    {/if}
  </div>
</section>
