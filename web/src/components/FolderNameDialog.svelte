<script lang="ts">
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let title = $state("");
  let value = $state("");
  let submitLabel = $state("Save");
  let resolve: ((value: string | null) => void) | undefined;

  export function ask(options: {
    title: string;
    value?: string;
    submitLabel?: string;
  }): Promise<string | null> {
    title = options.title;
    value = options.value ?? "";
    submitLabel = options.submitLabel ?? "Save";
    dialog.showModal();
    requestAnimationFrame(() => {
      input.focus();
      input.select();
    });
    return new Promise(answer => { resolve = answer; });
  }

  function finish(answer: string | null): void {
    dialog.close();
    resolve?.(answer);
    resolve = undefined;
  }
</script>

<dialog bind:this={dialog} class="site-dialog" aria-labelledby="folder-name-dialog-title"
  oncancel={(event) => { event.preventDefault(); finish(null); }}>
  <form class="dialog-form" onsubmit={(event) => {
    event.preventDefault();
    const name = value.trim();
    if (name) finish(name);
  }}>
    <h2 id="folder-name-dialog-title">{title}</h2>
    <label class="field"><span>Folder name</span><input bind:this={input} bind:value maxlength="64" required></label>
    <div class="actions">
      <button class="button" type="button" onclick={() => finish(null)}>Cancel</button>
      <button class="button primary" type="submit">{submitLabel}</button>
    </div>
  </form>
</dialog>
