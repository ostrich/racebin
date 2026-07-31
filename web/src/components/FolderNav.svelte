<script lang="ts">
  import type { FolderOverview } from "../types";
  import { navigate } from "../router";
  import Link from "./Link.svelte";

  let {
    overview,
    currentFolderId,
    unfiled,
    oncreate,
    onrename,
    ondelete
  }: {
    overview: FolderOverview;
    currentFolderId: number | null;
    unfiled: boolean;
    oncreate: () => void;
    onrename: (id: number, name: string) => void;
    ondelete: (id: number, name: string) => void;
  } = $props();
  let currentFolder = $derived(
    overview.items.find(folder => folder.id === currentFolderId)
  );

  function folderUrl(id?: number, uncategorized = false): string {
    const params = new URLSearchParams(location.search);
    params.delete("page");
    params.delete("folder_id");
    params.delete("unfiled");
    if (id) params.set("folder_id", String(id));
    if (uncategorized) params.set("unfiled", "true");
    return `/pastes${params.size ? `?${params}` : ""}`;
  }
</script>

<aside class="folder-nav" aria-label="Paste folders">
  <div class="folder-nav-heading"><strong>Folders</strong>
    <button type="button" onclick={oncreate}>New</button></div>
  <nav>
    <Link class={currentFolderId === null && !unfiled ? "current" : ""} href={folderUrl()}>
      <span>All pastes</span><small>{overview.total_count}</small>
    </Link>
    <Link class={unfiled ? "current" : ""} href={folderUrl(undefined, true)}>
      <span>Uncategorized</span><small>{overview.unfiled_count}</small>
    </Link>
    {#each overview.items as folder}
      <div class="folder-nav-row">
        <Link class={currentFolderId === folder.id ? "current" : ""} href={folderUrl(folder.id)}>
          <span>{folder.name}</span><small>{folder.paste_count}</small>
        </Link>
        <button type="button" title={`Rename ${folder.name}`}
          onclick={() => onrename(folder.id, folder.name)}>Rename</button>
        <button type="button" title={`Delete ${folder.name}`}
          onclick={() => ondelete(folder.id, folder.name)}>Delete</button>
      </div>
    {/each}
  </nav>
  <div class="folder-mobile-controls">
    <label class="folder-mobile-select"><span>Folder</span>
      <select value={unfiled ? "unfiled" : currentFolderId ? String(currentFolderId) : ""}
        onchange={(event) => {
          const value = event.currentTarget.value;
          void navigate(value === "unfiled" ? folderUrl(undefined, true)
            : value ? folderUrl(Number(value)) : folderUrl());
        }}>
        <option value="">All pastes ({overview.total_count})</option>
        <option value="unfiled">Uncategorized ({overview.unfiled_count})</option>
        {#each overview.items as folder}<option value={folder.id}>{folder.name} ({folder.paste_count})</option>{/each}
      </select>
    </label>
    <button type="button" onclick={oncreate}>New folder</button>
    {#if currentFolder}
      <button type="button" onclick={() => onrename(currentFolder.id, currentFolder.name)}>Rename</button>
      <button type="button" onclick={() => ondelete(currentFolder.id, currentFolder.name)}>Delete</button>
    {/if}
  </div>
</aside>
