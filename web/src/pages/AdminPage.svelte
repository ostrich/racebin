<script lang="ts">
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatDate } from "../format";
  import { showNotice } from "../notices";
  import type { ApiKey, User } from "../types";

  type Section = "users" | "invitations" | "keys";
  type Invitation = {
    id: number;
    token_prefix: string;
    expires_at: number;
    status: string;
    redeemed_by_username: string | null;
  };

  let section = $state<Section | null>(null);
  let users = $state<User[]>([]);
  let invitations = $state<Invitation[]>([]);
  let keys = $state<ApiKey[]>([]);
  let loading = $state(false);

  async function load(target: Section): Promise<void> {
    section = target;
    loading = true;
    try {
      if (target === "users") users = await requestApi<User[]>("/admin/users");
      else if (target === "invitations") invitations = await requestApi<Invitation[]>("/admin/invitations");
      else {
        [keys, users] = await Promise.all([
          requestApi<ApiKey[]>("/admin/api-keys"),
          requestApi<User[]>("/admin/users")
        ]);
      }
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Unable to load administration data", "error");
    } finally {
      loading = false;
    }
  }

  async function patchUser(user: User, patch: { role?: string; enabled?: boolean }): Promise<void> {
    try {
      await requestApi(`/admin/users/${user.id}`, {
        method: "PATCH",
        body: JSON.stringify(patch)
      });
      Object.assign(user, patch);
      users = [...users];
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
      await load("users");
    }
  }

  async function createInvitation(): Promise<void> {
    try {
      const invitation = await requestApi<{ url: string }>("/admin/invitations", { method: "POST" });
      await navigator.clipboard.writeText(invitation.url);
      showNotice("Invitation link copied.");
      await load("invitations");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function revoke(invitation: Invitation): Promise<void> {
    try {
      await requestApi(`/admin/invitations/${invitation.id}`, { method: "DELETE" });
      await load("invitations");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function toggleKey(key: ApiKey, enabled: boolean): Promise<void> {
    try {
      await requestApi(`/admin/api-keys/${key.id}`, {
        method: "PATCH", body: JSON.stringify({ enabled })
      });
      key.enabled = enabled;
      keys = [...keys];
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
      await load("keys");
    }
  }

  async function deleteKey(key: ApiKey): Promise<void> {
    if (!confirm("Delete this API key permanently?")) return;
    try {
      await requestApi(`/admin/api-keys/${key.id}`, { method: "DELETE" });
      keys = keys.filter(candidate => candidate.id !== key.id);
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  function ownerName(key: ApiKey): string {
    return key.user_id === null
      ? "No owner"
      : users.find(user => user.id === key.user_id)?.username ?? `User #${key.user_id}`;
  }
</script>

<section>
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Admin</h1></div></div>
  <div class="admin-links">
    <Link href="/admin/pastes"><Icon name="file-text"/><div><strong>All pastes</strong><span>Search, filter, edit, and remove pastes</span></div></Link>
    <button type="button" onclick={() => load("users")}><Icon name="user-round"/><div><strong>Users</strong><span>Roles and account access</span></div></button>
    <button type="button" onclick={() => load("invitations")}><Icon name="plus"/><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" onclick={() => load("keys")}><Icon name="key-round"/><div><strong>API keys</strong><span>Review and revoke keys</span></div></button>
  </div>
  {#if section}
    <section class="panel" aria-busy={loading}>
      {#if loading}<p class="muted">Loading…</p>
      {:else if section === "users"}
        <h2>Users</h2><div class="table">
          {#each users as user (user.id)}
            <div>
              <strong>{user.username}</strong>
              <select aria-label={`Role for ${user.username}`} value={user.role}
                onchange={(event) => patchUser(user, { role: event.currentTarget.value })}>
                <option value="user">User</option><option value="admin">Admin</option>
              </select>
              <label class="check"><input type="checkbox" checked={user.enabled}
                onchange={(event) => patchUser(user, { enabled: event.currentTarget.checked })}/><span>Enabled</span></label>
            </div>
          {/each}
        </div>
      {:else if section === "invitations"}
        <div class="section-heading"><h2>Invitations</h2><button class="button primary" type="button" onclick={createInvitation}>Create invitation</button></div>
        <div class="table">
          {#each invitations as invitation (invitation.id)}
            <div><code>{invitation.token_prefix}…</code>
              <span>{invitation.status === "Redeemed" && invitation.redeemed_by_username
                ? `Redeemed by ${invitation.redeemed_by_username}`
                : `${invitation.status} · ${formatDate(invitation.expires_at)}`}</span>
              {#if invitation.status === "Active"}<button class="button" type="button" onclick={() => revoke(invitation)}>Revoke</button>{:else}<span></span>{/if}
            </div>
          {/each}
        </div>
      {:else}
        <h2>API keys</h2><div class="table">
          {#each keys as key (key.id)}
            <div class="admin-key-row">
              <div class="admin-key-identity"><strong>{key.name}</strong><code>{key.token_prefix}</code></div>
              <div class="admin-key-access"><span>Owner: {ownerName(key)}</span>
                <div class="admin-key-scopes" aria-label="Privileges">
                  {#each key.scopes as scope}<code>{scope}</code>{:else}<span>No privileges</span>{/each}
                </div>
              </div>
              <div class="admin-key-actions">
                <label class="check"><input type="checkbox" checked={key.enabled}
                  onchange={(event) => toggleKey(key, event.currentTarget.checked)}/><span>Enabled</span></label>
                <button class="icon-button" title="Delete API key" aria-label={`Delete ${key.name}`}
                  type="button" onclick={() => deleteKey(key)}><Icon name="trash-2"/></button>
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </section>
  {/if}
</section>
