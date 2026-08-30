<script lang="ts">
  import {
    createInvitation,
    updateInvitationComment,
    type Invitation,
    type InvitationCreated
  } from "../api";
  import { showNotice } from "../app/notices";
  import Icon from "./Icon.svelte";

  let { oncreated }: { oncreated?: () => void | Promise<void> } = $props();
  let dialog: HTMLDialogElement;
  let comment = $state("");
  let result = $state<InvitationCreated | null>(null);
  let creating = $state(false);
  let editing = $state<Invitation | null>(null);
  let resultUrl = $derived(result ? new URL(result.url, location.origin).href : "");

  export function open(): void {
    comment = "";
    result = null;
    editing = null;
    creating = false;
    dialog.showModal();
  }

  export function edit(invitation: Invitation): void {
    comment = invitation.comment ?? "";
    result = null;
    editing = invitation;
    creating = false;
    dialog.showModal();
  }

  function close(): void {
    if (!creating) dialog.close();
  }

  async function create(): Promise<void> {
    creating = true;
    try {
      result = await createInvitation(comment.trim() || undefined);
      await oncreated?.();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to create invitation", "error");
    } finally {
      creating = false;
    }
  }

  async function save(): Promise<void> {
    if (!editing) return;
    creating = true;
    try {
      await updateInvitationComment(editing.id, comment.trim() || undefined);
      await oncreated?.();
      showNotice("Invitation note updated.");
      dialog.close();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to update invitation", "error");
    } finally {
      creating = false;
    }
  }

  async function copy(): Promise<void> {
    if (!result) return;
    await navigator.clipboard.writeText(resultUrl);
    showNotice("Invitation link copied.");
  }
</script>

<dialog
  bind:this={dialog}
  class="site-dialog invitation-dialog"
  aria-labelledby="invitation-dialog-title"
  oncancel={(event) => {
    if (creating) event.preventDefault();
  }}
>
  {#if result}
    <div class="dialog-form">
      <h2 id="invitation-dialog-title">Invitation created</h2>
      <p class="dialog-message">This one-use link expires in 24 hours.</p>
      <label class="field"
        ><span>Invitation link</span>
        <div class="copy-field">
          <input readonly value={resultUrl} /><button
            class="button primary"
            type="button"
            onclick={copy}><Icon name="link-2" /> Copy link</button
          >
        </div></label
      >
      {#if comment.trim()}<p class="invitation-comment">
          <strong>Note</strong><span>{comment.trim()}</span>
        </p>{/if}
      <div class="actions">
        <button class="button primary" type="button" onclick={close}>Done</button>
      </div>
    </div>
  {:else}
    <div class="dialog-form">
      <h2 id="invitation-dialog-title">{editing ? "Edit invitation note" : "Create invitation"}</h2>
      <p class="dialog-message">
        {editing
          ? "The note is visible only to administrators."
          : "The invitation can be used once and expires after 24 hours."}
      </p>
      <label class="field"
        ><span>Private note <small>Optional</small></span><input
          bind:value={comment}
          maxlength="200"
          placeholder="Who is this invitation for?"
        /></label
      >
      <p class="muted">Only administrators can see this note.</p>
      <div class="actions">
        <button class="button" type="button" disabled={creating} onclick={close}>Cancel</button>
        <button
          class="button primary"
          type="button"
          disabled={creating}
          onclick={editing ? save : create}
          >{creating ? "Saving…" : editing ? "Save note" : "Create invitation"}</button
        >
      </div>
    </div>
  {/if}
</dialog>
