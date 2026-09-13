<script lang="ts">
  let dialog: HTMLDialogElement;
  let input: HTMLInputElement;
  let title = $state("");
  let label = $state("");
  let value = $state("");
  let submitLabel = $state("Save");
  let maximumLength = $state(2048);
  let allowEmpty = $state(false);
  let resolve: ((value: string | null) => void) | undefined;

  export function ask(options: {
    title: string;
    label: string;
    value?: string;
    submitLabel?: string;
    maximumLength?: number;
    allowEmpty?: boolean;
  }): Promise<string | null> {
    title = options.title;
    label = options.label;
    value = options.value ?? "";
    submitLabel = options.submitLabel ?? "Save";
    maximumLength = options.maximumLength ?? 2048;
    allowEmpty = options.allowEmpty ?? false;
    dialog.showModal();
    requestAnimationFrame(() => {
      input.focus();
      input.select();
    });
    return new Promise((answer) => {
      resolve = answer;
    });
  }

  function finish(answer: string | null): void {
    dialog.close();
    resolve?.(answer);
    resolve = undefined;
  }
</script>

<dialog
  bind:this={dialog}
  class="site-dialog"
  aria-labelledby="text-input-dialog-title"
  oncancel={(event) => {
    event.preventDefault();
    finish(null);
  }}
>
  <form
    class="dialog-form"
    onsubmit={(event) => {
      event.preventDefault();
      const answer = value.trim();
      if (answer || allowEmpty) finish(answer);
    }}
  >
    <h2 id="text-input-dialog-title">{title}</h2>
    <label class="field"
      ><span>{label}</span><input bind:this={input} bind:value maxlength={maximumLength} /></label
    >
    <div class="actions">
      <button class="button" type="button" onclick={() => finish(null)}>Cancel</button>
      <button class="button primary" type="submit">{submitLabel}</button>
    </div>
  </form>
</dialog>
