<script lang="ts">
  import { showNotice } from "../app/notices";
  import Icon from "./Icon.svelte";

  let dialog: HTMLDialogElement;
  let token = $state("");

  export function open(value: string): void {
    token = value;
    dialog.showModal();
  }

  async function copy(): Promise<void> {
    await navigator.clipboard.writeText(token);
    showNotice("API key copied.");
  }
</script>

<dialog bind:this={dialog} class="site-dialog" aria-labelledby="secret-dialog-title">
  <div class="dialog-form">
    <h2 id="secret-dialog-title">API key created</h2>
    <p class="dialog-message">Store this key securely. It will not be shown again.</p>
    <label class="field">
      <span>API key</span>
      <div class="copy-field">
        <input readonly value={token} />
        <button class="button primary" type="button" onclick={copy}
          ><Icon name="copy" /> Copy</button
        >
      </div>
    </label>
    <div class="actions">
      <button class="button primary" type="button" onclick={() => dialog.close()}>Done</button>
    </div>
  </div>
</dialog>
