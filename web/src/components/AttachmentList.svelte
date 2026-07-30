<script lang="ts">
  import { requestApi } from "../api";
  import { showNotice } from "../notices";
  import type { Attachment } from "../types";
  import Icon from "./Icon.svelte";

  let {
    pasteId,
    attachments,
    canDelete = false,
    editing = false,
    ondelete
  }: {
    pasteId: string;
    attachments: Attachment[];
    canDelete?: boolean;
    editing?: boolean;
    ondelete?: (attachment: Attachment) => void;
  } = $props();

  async function remove(attachment: Attachment): Promise<void> {
    const suffix = editing
      ? "\n\nThis takes effect immediately, even if you cancel editing."
      : "";
    if (!confirm(`Delete this attachment permanently?${suffix}`)) return;
    try {
      await requestApi(`/pastes/${encodeURIComponent(pasteId)}/attachments/${attachment.id}`, {
        method: "DELETE"
      });
      ondelete?.(attachment);
      showNotice("Attachment deleted.");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }
</script>

<div class="attachments">
  {#each attachments as attachment (attachment.id)}
    <div class="attachment-row">
      <a href={`/api/v1/pastes/${encodeURIComponent(pasteId)}/attachments/${attachment.id}`}>
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
