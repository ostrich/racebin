<script lang="ts">
  import { requestApi } from "../api";
  import Link from "../components/Link.svelte";
  import { loadSession } from "../session";

  let { token }: { token: string } = $props();
  let password = $state("");
  let confirmation = $state("");
  let error = $state("");
  let saving = $state(false);
  let complete = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    error = "";
    if (password !== confirmation) { error = "Passwords do not match."; return; }
    saving = true;
    try {
      await requestApi(`/password-resets/${encodeURIComponent(token)}`, {
        method: "POST", body: JSON.stringify({ new_password: password })
      });
      await loadSession();
      complete = true;
      password = confirmation = "";
    } catch (reason) {
      error = reason instanceof Error ? reason.message : "Unable to reset password";
    } finally { saving = false; }
  }
</script>

<section class="auth-card panel">
  {#if complete}
    <p class="eyebrow">Account recovery</p><h1>Password reset</h1>
    <p>Your password has been changed and existing sessions have been signed out.</p>
    <Link class="button primary" href="/login">Log in</Link>
  {:else}
    <p class="eyebrow">Account recovery</p><h1>Choose a new password</h1>
    <p class="muted">This one-time link expires one hour after it was created.</p>
    <form class="stack" onsubmit={submit}>
      <label><span>New password</span><input type="password" minlength="12" autocomplete="new-password" bind:value={password} required></label>
      <label><span>Confirm password</span><input type="password" minlength="12" autocomplete="new-password" bind:value={confirmation} required></label>
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
      <button class="button primary" type="submit" disabled={saving}>{saving ? "Resetting…" : "Reset password"}</button>
    </form>
  {/if}
</section>
