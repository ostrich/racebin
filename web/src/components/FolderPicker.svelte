<script lang="ts">
  import type { FolderOverview } from "../types";
  import Icon from "./Icon.svelte";

  let {
    overview,
    mode,
    label,
    currentFolderId = null,
    unfiled = false,
    disabled = false,
    onselect,
    oncreate,
    onrename,
    ondelete
  }: {
    overview: FolderOverview;
    mode: "browse" | "move";
    label: string;
    currentFolderId?: number | null;
    unfiled?: boolean;
    disabled?: boolean;
    onselect: (folderId: number | null, unfiled: boolean) => void;
    oncreate?: () => void;
    onrename?: (id: number, name: string) => void;
    ondelete?: (id: number, name: string) => void;
  } = $props();

  let root = $state<HTMLElement>();
  let searchInput = $state<HTMLInputElement>();
  let open = $state(false);
  let query = $state("");
  let managedFolderId = $state<number | null>(null);
  let filteredFolders = $derived(overview.items.filter(folder =>
    folder.name.toLowerCase().includes(query.trim().toLowerCase())
  ));
  let popoverId = $derived(`folder-picker-${mode}`);

  function toggle(): void {
    if (disabled) return;
    open = !open;
    query = "";
    managedFolderId = null;
    if (open) requestAnimationFrame(() => searchInput?.focus());
  }

  function close(returnFocus = false): void {
    if (!open) return;
    open = false;
    query = "";
    managedFolderId = null;
    if (returnFocus) requestAnimationFrame(() =>
      root?.querySelector<HTMLButtonElement>(".folder-picker-trigger")?.focus()
    );
  }

  function choose(folderId: number | null, isUnfiled = false): void {
    close();
    onselect(folderId, isUnfiled);
  }
</script>

<svelte:window onclick={(event) => {
    if (open && root && !root.contains(event.target as Node)) close();
  }} onkeydown={(event) => { if (event.key === "Escape") close(true); }}/>

<div class="folder-picker" bind:this={root}>
  <button class="button folder-picker-trigger" type="button" {disabled}
    aria-haspopup="dialog" aria-expanded={open} aria-controls={popoverId} onclick={toggle}>
    <Icon name="folder"/><span>{label}</span><Icon name="chevron-down"/>
  </button>
  {#if open}
    <div class="folder-picker-popover" id={popoverId} role="dialog"
      aria-label={mode === "browse" ? "Browse folders" : "Move selected pastes"}>
      <label class="folder-picker-search">
        <span class="visually-hidden">Filter folders</span>
        <Icon name="search"/><input bind:this={searchInput} bind:value={query} type="search" placeholder="Filter folders">
      </label>
      <div class="folder-picker-list">
        {#if mode === "browse"}
          <button class:current={currentFolderId === null && !unfiled} class="folder-picker-choice"
            type="button" onclick={() => choose(null)}>
            <span>All pastes</span><small>{overview.total_count}</small>
          </button>
        {/if}
        <button class:current={mode === "browse" && unfiled} class="folder-picker-choice"
          type="button" onclick={() => choose(null, true)}>
          <span>Uncategorized</span><small>{overview.unfiled_count}</small>
        </button>
        {#each filteredFolders as folder (folder.id)}
          <div class="folder-picker-row">
            <button class:current={mode === "browse" && currentFolderId === folder.id}
              class="folder-picker-choice" type="button" onclick={() => choose(folder.id)}>
              <span>{folder.name}</span><small>{folder.paste_count}</small>
            </button>
            {#if mode === "browse"}
              <button class="folder-picker-manage" type="button" aria-label={`Manage ${folder.name}`}
                aria-expanded={managedFolderId === folder.id}
                onclick={() => { managedFolderId = managedFolderId === folder.id ? null : folder.id; }}>
                <Icon name="more-horizontal"/>
              </button>
              {#if managedFolderId === folder.id}
                <div class="folder-picker-row-actions">
                  <button type="button" onclick={() => { close(); onrename?.(folder.id, folder.name); }}>Rename</button>
                  <button class="danger" type="button" onclick={() => { close(); ondelete?.(folder.id, folder.name); }}>Delete</button>
                </div>
              {/if}
            {/if}
          </div>
        {/each}
        {#if !filteredFolders.length && query}<p class="folder-picker-empty">No matching folders</p>{/if}
      </div>
      {#if mode === "browse" && oncreate}
        <button class="folder-picker-create" type="button" onclick={() => { close(); oncreate(); }}>
          <Icon name="plus"/> New folder
        </button>
      {/if}
    </div>
  {/if}
</div>
