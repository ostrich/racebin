<script lang="ts">
  import { requestApi } from "../api";
  import Link from "../components/Link.svelte";
  import { showNotice } from "../notices";
  import { guardUnsavedChanges, navigate } from "../router";
  import { replaceSession } from "../session";

  let submitting = $state(false);
  let dirty = $state(false);
  $effect(() => {
    guardUnsavedChanges(() => dirty);
    return () => guardUnsavedChanges();
  });

  async function submit(event: SubmitEvent): Promise<void> {
    const data = new FormData(event.currentTarget as HTMLFormElement);
    submitting = true;
    try {
      await requestApi("/account/password", {
        method: "PATCH",
        body: JSON.stringify({
          current_password: String(data.get("current_password") ?? ""),
          new_password: String(data.get("new_password") ?? "")
        })
      });
      dirty = false;
      guardUnsavedChanges();
      replaceSession({ authenticated: false });
      await navigate("/login");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Password update failed", "error");
    } finally {
      submitting = false;
    }
  }
</script>

<section class="auth"><form oninput={() => { dirty = true; }}
  onsubmit={(event) => { event.preventDefault(); void submit(event); }}>
  <p class="eyebrow">Security</p><h1>Change password</h1>
  <label><span>Current password</span><input type="password" name="current_password" autocomplete="current-password" required/></label>
  <label><span>New password</span><input type="password" name="new_password" minlength="12" autocomplete="new-password" required/></label>
  <div class="actions">
    <button class="button primary" type="submit" disabled={submitting}>{submitting ? "Updating…" : "Update password"}</button>
    <Link class="button" href="/account">Cancel</Link>
  </div>
</form></section>
