<script lang="ts">
  import { redeemInvitation } from "../api";
  import { showNotice } from "../app/notices";
  import { navigate } from "../navigation";
  import { refreshSession } from "../app/session";
  import { appState } from "../app/state";

  let { token }: { token: string } = $props();
  let submitting = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    submitting = true;
    try {
      await redeemInvitation(token, {
        username: String(data.get("username") ?? ""),
        password: String(data.get("password") ?? "")
      });
      try {
        await refreshSession();
        await navigate("/pastes");
      } catch (error) {
        showNotice(
          `Your account was created, but its session could not be loaded: ${error instanceof Error ? error.message : "refresh failed"}. Reload the page to continue.`,
          "error"
        );
      }
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Account creation failed", "error");
    } finally {
      submitting = false;
    }
  }
</script>

<section class="auth">
  <form
    onsubmit={(event) => {
      event.preventDefault();
      void submit(event);
    }}
  >
    <p class="eyebrow">Invitation</p>
    <h1>Create your account</h1>
    <label class="field"
      ><span>Username</span><input name="username" autocomplete="username" required /></label
    >
    <label class="field"
      ><span>Password</span><input
        type="password"
        name="password"
        minlength={$appState.config.minimum_password_characters}
        autocomplete="new-password"
        required
      /></label
    >
    <button class="button primary" type="submit" disabled={submitting}
      >{submitting ? "Creating…" : "Create account"}</button
    >
  </form>
</section>
