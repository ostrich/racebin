<script lang="ts">
  import { login } from "../api";
  import { showNotice } from "../app/notices";
  import { navigate } from "../navigation";
  import { loadSession } from "../app/session";

  let submitting = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    submitting = true;
    try {
      await login({
        username: String(data.get("username") ?? ""),
        password: String(data.get("password") ?? ""),
        remember: data.has("remember")
      });
      await loadSession();
      await navigate("/pastes");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Login failed", "error");
    } finally {
      submitting = false;
    }
  }
</script>

<section class="auth"><form onsubmit={(event) => { event.preventDefault(); void submit(event); }}>
  <p class="eyebrow">Account</p><h1>Log in</h1>
  <label class="field"><span>Username</span><input name="username" autocomplete="username" required/></label>
  <label class="field"><span>Password</span><input type="password" name="password" autocomplete="current-password" required/></label>
  <label class="check"><input type="checkbox" name="remember"/><span>Keep me signed in</span></label>
  <button class="button primary" type="submit" disabled={submitting}>{submitting ? "Logging in…" : "Log in"}</button>
</form></section>
