<script lang="ts">
  import { formatByteSize } from "../format";
  import { showNotice } from "../notices";
  import Icon from "./Icon.svelte";

  let {
    files = $bindable<File[]>([]),
    existingCount = 0,
    maxFiles,
    maxBytes
  }: {
    files?: File[];
    existingCount?: number;
    maxFiles: number;
    maxBytes: number;
  } = $props();

  let input: HTMLInputElement;
  let dragging = $state(false);
  let totalBytes = $derived(files.reduce((total, file) => total + file.size, 0));

  function identity(file: File): string {
    return `${file.name}:${file.size}:${file.lastModified}`;
  }

  function addFiles(selection: FileList | File[]): void {
    const known = new Set(files.map(identity));
    const additions: File[] = [];
    let bytes = totalBytes;
    let rejectedForCount = false;
    let rejectedForSize = false;

    for (const file of Array.from(selection)) {
      const key = identity(file);
      if (known.has(key)) continue;
      if (existingCount + files.length + additions.length >= maxFiles) {
        rejectedForCount = true;
        continue;
      }
      if (bytes + file.size > maxBytes) {
        rejectedForSize = true;
        continue;
      }
      known.add(key);
      additions.push(file);
      bytes += file.size;
    }

    if (additions.length) files = [...files, ...additions];
    if (rejectedForCount) showNotice(`A paste can have at most ${maxFiles} attachments.`, "error");
    else if (rejectedForSize) showNotice("Selected attachments exceed the server upload limit.", "error");
  }

  function selected(): void {
    addFiles(input.files ?? []);
    input.value = "";
  }

  function dropped(event: DragEvent): void {
    event.preventDefault();
    dragging = false;
    if (event.dataTransfer?.files.length) addFiles(event.dataTransfer.files);
  }

  function remove(index: number): void {
    files = files.filter((_, candidate) => candidate !== index);
  }
</script>

<div class="attachment-picker">
  <div class="attachment-drop-zone" class:dragging role="group" aria-label="Attachment drop zone"
    ondragenter={(event) => { event.preventDefault(); dragging = true; }}
    ondragover={(event) => event.preventDefault()}
    ondragleave={(event) => {
      if (!(event.currentTarget as HTMLElement).contains(event.relatedTarget as Node | null)) dragging = false;
    }}
    ondrop={dropped}>
    <div>
      <strong>Add attachments</strong>
      <small>Choose files or drag them here</small>
    </div>
    <button class="button" type="button" onclick={() => input.click()}>Choose files</button>
    <input class="visually-hidden" bind:this={input} type="file" multiple tabindex="-1"
      aria-hidden="true" aria-label="Add attachments" onchange={selected}/>
  </div>

  {#if files.length}
    <div class="attachment-queue" role="region" aria-label="Selected attachments">
      {#each files as file, index (identity(file))}
        <div class="attachment-queue-row">
          <Icon name="file-text"/>
          <span title={file.name}>{file.name}</span>
          <small>{formatByteSize(file.size)}</small>
          <button class="icon-button" type="button" title="Remove attachment"
            aria-label={`Remove ${file.name}`} onclick={() => remove(index)}>
            <Icon name="trash-2"/>
          </button>
        </div>
      {/each}
      <small class="attachment-queue-summary">
        {files.length} {files.length === 1 ? "file" : "files"} · {formatByteSize(totalBytes)} selected
      </small>
    </div>
  {/if}

  <small class="attachment-picker-limits">
    Up to {maxFiles} files; combined upload limit: {formatByteSize(maxBytes)}
  </small>
</div>
