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

  let trigger: HTMLButtonElement;
  let popup: HTMLDivElement;
  let open = $state(false);
  let query = $state("");
  let managedFolderId = $state<number | null>(null);
  let filteredFolders = $derived(
    overview.items.filter((folder) =>
      folder.name.toLowerCase().includes(query.trim().toLowerCase())
    )
  );
  let popoverId = $derived(`folder-picker-${mode}`);

  function position(): void {
    if (!open) return;
    const bounds = trigger.getBoundingClientRect();
    const width = popup.offsetWidth;
    const height = popup.offsetHeight;
    popup.style.left = `${Math.max(16, Math.min(bounds.left, innerWidth - width - 16))}px`;
    popup.style.top = `${Math.min(bounds.bottom + 6, Math.max(16, innerHeight - height - 16))}px`;
  }

  function close(): void {
    if (!open) return;
    popup.hidePopover();
  }

  function choose(folderId: number | null, isUnfiled = false): void {
    close();
    onselect(folderId, isUnfiled);
  }
</script>

<svelte:window onresize={position} onscroll={position} />

<div class="folder-picker">
  <button
    class="button folder-picker-trigger"
    type="button"
    {disabled}
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-controls={popoverId}
    bind:this={trigger}
    popovertarget={popoverId}
  >
    <Icon name="folder" /><span>{label}</span><Icon name="chevron-down" />
  </button>
  <div
    bind:this={popup}
    popover="auto"
    ontoggle={(event) => {
      open = event.newState === "open";
      query = "";
      managedFolderId = null;
      position();
      if (open) popup.querySelector<HTMLInputElement>("input")?.focus();
    }}
    class="folder-picker-popover"
    id={popoverId}
    role="dialog"
    aria-label={mode === "browse" ? "Browse folders" : "Move selected pastes"}
  >
    <label class="folder-picker-search">
      <span class="visually-hidden">Filter folders</span>
      <Icon name="search" /><input bind:value={query} type="search" placeholder="Filter folders" />
    </label>
    <div class="folder-picker-list">
      {#if mode === "browse"}
        <button
          class:current={currentFolderId === null && !unfiled}
          class="folder-picker-choice"
          type="button"
          onclick={() => choose(null)}
        >
          <span>My pastes</span><small>{overview.total_count}</small>
        </button>
      {/if}
      <button
        class:current={mode === "browse" && unfiled}
        class="folder-picker-choice"
        type="button"
        onclick={() => choose(null, true)}
      >
        <span>Uncategorized</span><small>{overview.unfiled_count}</small>
      </button>
      {#each filteredFolders as folder (folder.id)}
        <div class="folder-picker-row" class:manageable={mode === "browse"}>
          <button
            class:current={mode === "browse" && currentFolderId === folder.id}
            class="folder-picker-choice"
            type="button"
            onclick={() => choose(folder.id)}
          >
            <span>{folder.name}</span><small>{folder.paste_count}</small>
          </button>
          {#if mode === "browse"}
            <button
              class="folder-picker-manage"
              type="button"
              aria-label={`Manage ${folder.name}`}
              aria-expanded={managedFolderId === folder.id}
              onclick={() => {
                managedFolderId = managedFolderId === folder.id ? null : folder.id;
              }}
            >
              <Icon name="more-horizontal" />
            </button>
            {#if managedFolderId === folder.id}
              <div class="folder-picker-row-actions">
                <button
                  type="button"
                  onclick={() => {
                    close();
                    onrename?.(folder.id, folder.name);
                  }}>Rename</button
                >
                <button
                  class="danger"
                  type="button"
                  onclick={() => {
                    close();
                    ondelete?.(folder.id, folder.name);
                  }}>Delete</button
                >
              </div>
            {/if}
          {/if}
        </div>
      {/each}
      {#if !filteredFolders.length && query}<p class="folder-picker-empty">
          No matching folders
        </p>{/if}
    </div>
    {#if mode === "browse" && oncreate}
      <button
        class="folder-picker-create"
        type="button"
        onclick={() => {
          close();
          oncreate();
        }}
      >
        <Icon name="plus" /> New folder
      </button>
    {/if}
  </div>
</div>
