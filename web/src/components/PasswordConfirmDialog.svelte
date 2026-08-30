<script lang="ts">
  let dialog: HTMLDialogElement;
  let passwordInput: HTMLInputElement;
  let password = $state("");
  let resolve: ((password: string | null) => void) | undefined;

  export function ask(): Promise<string | null> {
    password = "";
    dialog.showModal();
    queueMicrotask(() => passwordInput.focus());
    return new Promise((answer) => {
      resolve = answer;
    });
  }

  function finish(value: string | null): void {
    dialog.close();
    resolve?.(value);
    resolve = undefined;
  }
</script>

<dialog
  bind:this={dialog}
  class="site-dialog"
  aria-labelledby="password-confirm-dialog-title"
  oncancel={(event) => {
    event.preventDefault();
    finish(null);
  }}
>
  <form
    class="dialog-form"
    onsubmit={(event) => {
      event.preventDefault();
      if (password) finish(password);
    }}
  >
    <div>
      <h2 id="password-confirm-dialog-title">Confirm your password</h2>
      <p class="muted">Sensitive owner actions require a recent password confirmation.</p>
    </div>
    <label class="field"
      ><span>Password</span><input
        bind:this={passwordInput}
        type="password"
        autocomplete="current-password"
        bind:value={password}
      /></label
    >
    <div class="actions">
      <button class="button" type="button" onclick={() => finish(null)}>Cancel</button><button
        class="button primary"
        type="submit"
        disabled={!password}>Continue</button
      >
    </div>
  </form>
</dialog>
