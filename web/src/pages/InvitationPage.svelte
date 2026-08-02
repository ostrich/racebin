<script lang="ts">
  import { requestApi } from "../api";
  import { showNotice } from "../notices";
  import { navigate } from "../navigation";
  import { loadSession } from "../session";
  import { appState } from "../state";

  let { token }: { token: string } = $props();
  let submitting = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    submitting = true;
    try {
      await requestApi(`/invitations/${encodeURIComponent(token)}/redeem`, {
        method: "POST",
        body: JSON.stringify({
          username: String(data.get("username") ?? ""),
          password: String(data.get("password") ?? "")
        })
      });
      await loadSession();
      await navigate("/pastes");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Account creation failed", "error");
    } finally {
      submitting = false;
    }
  }
</script>

<section class="auth"><form onsubmit={(event) => { event.preventDefault(); void submit(event); }}>
  <p class="eyebrow">Invitation</p><h1>Create your account</h1>
  <label><span>Username</span><input name="username" autocomplete="username" required/></label>
  <label><span>Password</span><input type="password" name="password" minlength={$appState.config.minimum_password_characters} autocomplete="new-password" required/></label>
  <button class="button primary" type="submit" disabled={submitting}>{submitting ? "Creating…" : "Create account"}</button>
</form></section>
