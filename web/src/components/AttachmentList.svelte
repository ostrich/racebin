<script lang="ts">
  import { deleteAttachment } from "../api";
  import { formatByteSize } from "../format";
  import { showNotice } from "../notices";
  import { confirmAction } from "../confirmations";
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
    if (!(await confirmAction({
      title: "Delete attachment?",
      message: `This attachment will be permanently deleted.${suffix}`,
      confirmLabel: "Delete attachment",
      dangerous: true
    }))) return;
    try {
      const result = await deleteAttachment(pasteId, attachment.id, etag ?? "*");
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
        <span title={attachment.filename}>{attachment.filename}</span>
        <small>{formatByteSize(attachment.size_bytes)}</small>
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
