<script lang="ts">
  let dialog: HTMLDialogElement;
  let target = $state("");
  let preview = $state("");
  let resolve: ((answer: boolean) => void) | undefined;

  export function ask(targetKind: string, content: string): Promise<boolean> {
    target = targetKind;
    preview = content.slice(0, 4000);
    dialog.showModal();
    return new Promise(answer => { resolve = answer; });
  }

  function finish(answer: boolean): void {
    dialog.close();
    resolve?.(answer);
    resolve = undefined;
  }
</script>

<dialog bind:this={dialog} class="conversion-dialog" oncancel={(event) => {
  event.preventDefault(); finish(false);
}}>
  <h2>Convert to {target.replace("_", " ")}?</h2>
  <p class="muted">{target === "text" ? "Formatting will be removed when you save." : "Review the converted text before continuing."}</p>
  <pre>{preview}</pre>
  <div class="actions">
    <button class="button" type="button" onclick={() => finish(false)}>Cancel</button>
    <button class="button primary" type="button" onclick={() => finish(true)}>Convert</button>
  </div>
</dialog>
