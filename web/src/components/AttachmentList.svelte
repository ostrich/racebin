<script lang="ts">
  import { requestApiResult } from "../api";
  import { showNotice } from "../notices";
  import type { Attachment } from "../types";
  import Icon from "./Icon.svelte";

  let {
    pasteId,
    attachments,
    canDelete = false,
    editing = false,
    etag,
    ondelete
  }: {
    pasteId: string;
    attachments: Attachment[];
    canDelete?: boolean;
    editing?: boolean;
    etag?: string;
    ondelete?: (attachment: Attachment, etag: string | null) => void;
  } = $props();

  async function remove(attachment: Attachment): Promise<void> {
    const suffix = editing
      ? "\n\nThis takes effect immediately, even if you cancel editing."
      : "";
    if (!confirm(`Delete this attachment permanently?${suffix}`)) return;
    try {
      const result = await requestApiResult<void>(`/pastes/${encodeURIComponent(pasteId)}/attachments/${attachment.id}`, {
        method: "DELETE",
        headers: { "If-Match": etag ?? "*" }
      });
      ondelete?.(attachment, result.etag);
      showNotice("Attachment deleted.");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }
</script>

<div class="attachments">
  {#each attachments as attachment (attachment.id)}
    <div class="attachment-row">
      <a href={attachment.url}>
        <Icon name="file-text"/>
        <span>{attachment.filename}</span>
        <small>{attachment.size_bytes.toLocaleString()} bytes</small>
      </a>
      {#if canDelete}
        <button class="icon-button" type="button" title="Delete attachment"
          aria-label={`Delete ${attachment.filename}`} onclick={() => remove(attachment)}>
          <Icon name="trash-2"/>
        </button>
      {/if}
    </div>
  {/each}
</div>
