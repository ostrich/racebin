<script lang="ts">
  import { formatByteSize, formatDate, pasteFormatLabel } from "../format";
  import type { Paste } from "../types";
  import Link from "./Link.svelte";

  let {
    paste,
    filterable = false,
    filterUrl,
    folderNames,
    includeDate = true
  }: {
    paste: Paste;
    filterable?: boolean;
    filterUrl?: (key: string, value: string) => string;
    folderNames?: Map<number, string>;
    includeDate?: boolean;
  } = $props();

  const formatFilter = $derived(
    paste.format === "text"
      ? (["language", paste.language] as const)
      : (["format", paste.format] as const)
  );
</script>

<div class="paste-meta">
  {#if filterable && filterUrl}
    <Link class="meta-badge" href={filterUrl(formatFilter[0], formatFilter[1])}
      >{pasteFormatLabel(paste)}</Link
    >
    <Link class="meta-badge" href={filterUrl("visibility", paste.visibility)}
      >{paste.visibility}</Link
    >
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
      <span class="meta-detail"
        >{paste.attachment_count} attachment{paste.attachment_count === 1 ? "" : "s"}</span
      >
    {/if}
  {/if}
  <span class="meta-detail">{formatByteSize(paste.size_bytes)}</span>
  <span class="meta-detail">{paste.read_count} view{paste.read_count === 1 ? "" : "s"}</span>
  {#if includeDate}
    <time class="meta-detail" datetime={new Date(paste.created_at * 1000).toISOString()}
      >Created {formatDate(paste.created_at)}</time
    >
    {#if paste.modified_at}
      <time class="meta-detail" datetime={new Date(paste.modified_at * 1000).toISOString()}
        >Modified {formatDate(paste.modified_at)}</time
      >
    {/if}
  {/if}
</div>
