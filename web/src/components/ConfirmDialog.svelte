<script lang="ts">
  let dialog: HTMLDialogElement;
  let title = $state("");
  let message = $state("");
  let confirmLabel = $state("Confirm");
  let dangerous = $state(false);
  let resolve: ((answer: boolean) => void) | undefined;

  export function ask(options: {
    title: string;
    message: string;
    confirmLabel?: string;
    dangerous?: boolean;
  }): Promise<boolean> {
    title = options.title;
    message = options.message;
    confirmLabel = options.confirmLabel ?? "Confirm";
    dangerous = options.dangerous ?? false;
    dialog.showModal();
    return new Promise(answer => {
      resolve = answer;
    });
  }

  function finish(answer: boolean): void {
    dialog.close();
    resolve?.(answer);
    resolve = undefined;
  }
</script>

<dialog bind:this={dialog} class="conversion-dialog" oncancel={(event) => {
  event.preventDefault();
  finish(false);
}}>
  <h2>{title}</h2>
  <p class="muted">{message}</p>
  <div class="actions">
    <button class="button" type="button" onclick={() => finish(false)}>Cancel</button>
    <button class:danger={dangerous} class:primary={!dangerous} class="button" type="button"
      onclick={() => finish(true)}>{confirmLabel}</button>
  </div>
</dialog>
