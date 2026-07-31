<script lang="ts">
  import { onMount } from "svelte";
  import type { FolderOverview } from "../types";
  import { navigate } from "../router";
  import Icon from "./Icon.svelte";
  import Link from "./Link.svelte";

  let {
    overview,
    currentFolderId,
    unfiled,
    collapsed = $bindable(false),
    oncreate,
    onrename,
    ondelete
  }: {
    overview: FolderOverview;
    currentFolderId: number | null;
    unfiled: boolean;
    collapsed?: boolean;
    oncreate: () => void;
    onrename: (id: number, name: string) => void;
    ondelete: (id: number, name: string) => void;
  } = $props();
  const collapseStorageKey = "racebin.folderSidebarCollapsed";
  let openMenuId = $state<number | null>(null);
  let currentFolder = $derived(
    overview.items.find(folder => folder.id === currentFolderId)
  );

  onMount(() => {
    collapsed = localStorage.getItem(collapseStorageKey) === "true";
  });

  function toggleCollapsed(): void {
    collapsed = !collapsed;
    localStorage.setItem(collapseStorageKey, String(collapsed));
    openMenuId = null;
  }

  function closeMenu(returnFocus = false): void {
    const closingId = openMenuId;
    openMenuId = null;
    if (returnFocus && closingId !== null) {
      requestAnimationFrame(() =>
        document.querySelector<HTMLButtonElement>(`[data-folder-menu="${closingId}"]`)?.focus()
      );
    }
  }

  function openFolderMenu(id: number): void {
    if (openMenuId === id) {
      closeMenu();
      return;
    }
    openMenuId = id;
    requestAnimationFrame(() =>
      document.querySelector<HTMLButtonElement>(`#folder-menu-${id} [role="menuitem"]`)?.focus()
    );
  }

  function navigateMenu(event: KeyboardEvent): void {
    if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) return;
    const menu = event.currentTarget as HTMLElement;
    const items = [...menu.querySelectorAll<HTMLButtonElement>('[role="menuitem"]')];
    const current = items.indexOf(document.activeElement as HTMLButtonElement);
    const target = event.key === "Home" ? 0
      : event.key === "End" ? items.length - 1
      : event.key === "ArrowDown" ? (current + 1) % items.length
      : (current - 1 + items.length) % items.length;
    event.preventDefault();
    items[target]?.focus();
  }

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

<svelte:window onclick={() => closeMenu()}
  onkeydown={(event) => { if (event.key === "Escape") closeMenu(true); }}/>

<aside class:collapsed class="folder-nav" aria-label="Paste folders">
  <div class="folder-nav-heading">
    <strong>Folders</strong>
    <div class="folder-nav-heading-actions">
      <button class="folder-new-button" type="button" onclick={oncreate}>New</button>
      <button class="folder-collapse-button" type="button" aria-expanded={!collapsed}
        aria-controls="folder-navigation"
        aria-label={collapsed ? "Expand folders" : "Collapse folders"}
        title={collapsed ? "Expand folders" : "Collapse folders"}
        onclick={toggleCollapsed}><Icon name={collapsed ? "panel-left-open" : "panel-left-close"}/></button>
    </div>
  </div>
  <nav id="folder-navigation">
    <div class="folder-nav-row">
      <Link class={currentFolderId === null && !unfiled ? "current" : ""} href={folderUrl()}>
        <span>All pastes</span><small>{overview.total_count}</small>
      </Link>
      <span class="folder-menu-space" aria-hidden="true"></span>
    </div>
    <div class="folder-nav-row">
      <Link class={unfiled ? "current" : ""} href={folderUrl(undefined, true)}>
        <span>Uncategorized</span><small>{overview.unfiled_count}</small>
      </Link>
      <span class="folder-menu-space" aria-hidden="true"></span>
    </div>
    {#each overview.items as folder}
      <div class="folder-nav-row">
        <Link class={currentFolderId === folder.id ? "current" : ""} href={folderUrl(folder.id)}
          title={folder.name}>
          <span>{folder.name}</span><small>{folder.paste_count}</small>
        </Link>
        <button class="folder-menu-button" type="button" data-folder-menu={folder.id}
          aria-label={`Manage ${folder.name}`} aria-expanded={openMenuId === folder.id}
          aria-controls={`folder-menu-${folder.id}`}
          onclick={(event) => {
            event.stopPropagation();
            openFolderMenu(folder.id);
          }}><Icon name="more-horizontal"/></button>
        {#if openMenuId === folder.id}
          <div class="folder-menu" id={`folder-menu-${folder.id}`} role="menu" tabindex="-1"
            onkeydown={navigateMenu}>
            <button type="button" role="menuitem"
              onclick={() => { closeMenu(); onrename(folder.id, folder.name); }}>Rename</button>
            <button class="danger" type="button" role="menuitem"
              onclick={() => { closeMenu(); ondelete(folder.id, folder.name); }}>Delete</button>
          </div>
        {/if}
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
