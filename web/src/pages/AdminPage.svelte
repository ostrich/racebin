<script lang="ts">
  import { requestApi } from "../api";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { formatDate } from "../format";
  import { showNotice } from "../notices";
  import type { ApiKey, User } from "../types";

  type Section = "invitations" | "keys";
  type Invitation = {
    id: number;
    token_prefix: string;
    expires_at: number;
    status: string;
    url: string | null;
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
      if (target === "invitations") invitations = await requestApi<Invitation[]>("/admin/invitations");
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

  async function createInvitation(): Promise<void> {
    try {
      const invitation = await requestApi<{ url: string }>("/admin/invitations", { method: "POST" });
      await copyInvitationUrl(invitation.url);
      showNotice("Invitation link copied.");
      await load("invitations");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Request failed", "error");
    }
  }

  async function copyInvitationUrl(url: string): Promise<void> {
    await navigator.clipboard.writeText(new URL(url, location.origin).href);
  }

  async function copyInvitation(invitation: Invitation): Promise<void> {
    if (!invitation.url) return;
    try {
      await copyInvitationUrl(invitation.url);
      showNotice("Invitation link copied.");
    } catch (error) {
      showNotice(error instanceof Error ? error.message : "Unable to copy invitation", "error");
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
    <Link href="/admin/users"><Icon name="user-round"/><div><strong>Users</strong><span>Accounts, recovery, roles, and access</span></div></Link>
    <button type="button" onclick={() => load("invitations")}><Icon name="plus"/><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" onclick={() => load("keys")}><Icon name="key-round"/><div><strong>API keys</strong><span>Review and revoke keys</span></div></button>
  </div>
  {#if section}
    <section class="panel" aria-busy={loading}>
      {#if loading}<p class="muted">Loading…</p>
      {:else if section === "invitations"}
        <div class="section-heading"><h2>Invitations</h2><button class="button primary" type="button" onclick={createInvitation}>Create invitation</button></div>
        <div class="table">
          {#each invitations as invitation (invitation.id)}
            <div><code>{invitation.token_prefix}…</code>
              <span>{invitation.status === "Redeemed" && invitation.redeemed_by_username
                ? `Redeemed by ${invitation.redeemed_by_username}`
                : `${invitation.status} · ${formatDate(invitation.expires_at)}`}</span>
              {#if invitation.status === "Active"}
                <div class="invitation-actions">
                  {#if invitation.url}<button class="icon-button" title="Copy invitation link"
                    aria-label={`Copy invitation ${invitation.token_prefix}`} type="button"
                    onclick={() => copyInvitation(invitation)}><Icon name="copy"/></button>
                  {:else}<button class="icon-button" title="URL unavailable; create a new invitation"
                    aria-label={`Invitation URL unavailable for ${invitation.token_prefix}`}
                    type="button" disabled><Icon name="copy"/></button>{/if}
                  <button class="button" type="button" onclick={() => revoke(invitation)}>Revoke</button>
                </div>
              {:else}<span></span>{/if}
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
