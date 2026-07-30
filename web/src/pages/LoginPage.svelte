<script lang="ts">
  import { requestApi } from "../api";
  import { showNotice } from "../notices";
  import { navigate } from "../router";
  import { loadSession } from "../session";

  let submitting = $state(false);

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    submitting = true;
    try {
      await requestApi("/session", {
        method: "POST",
        body: JSON.stringify({
          username: String(data.get("username") ?? ""),
          password: String(data.get("password") ?? ""),
          remember: data.has("remember")
        })
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
  <label><span>Username</span><input name="username" autocomplete="username" required/></label>
  <label><span>Password</span><input type="password" name="password" autocomplete="current-password" required/></label>
  <label class="check"><input type="checkbox" name="remember"/><span>Keep me signed in</span></label>
  <button class="button primary" type="submit" disabled={submitting}>{submitting ? "Logging in…" : "Log in"}</button>
</form></section>
