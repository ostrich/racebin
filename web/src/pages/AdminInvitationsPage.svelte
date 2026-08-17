<script lang="ts">
  import { onMount } from "svelte";
  import { createInvitation, listInvitations, revokeInvitation, type Invitation } from "../api";
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import { formatDate } from "../format";
  import { holdNavigation } from "../navigation";
  import { showNotice } from "../notices";
  import { appState } from "../state";

  let invitations = $state<Invitation[]>([]);
  let error = $state("");
  const initialLoadReady = holdNavigation();

  async function load(): Promise<void> {
    invitations = await listInvitations();
  }

  onMount(() => {
    void load()
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load invitations"; })
      .finally(initialLoadReady);
  });

  async function create(): Promise<void> {
    try {
      const invitation = await createInvitation();
      await navigator.clipboard.writeText(new URL(invitation.url, location.origin).href);
      showNotice("Invitation link copied.");
      await load();
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to create invitation", "error");
    }
  }

  async function copy(invitation: Invitation): Promise<void> {
    if (!invitation.url) return;
    await navigator.clipboard.writeText(new URL(invitation.url, location.origin).href);
    showNotice("Invitation link copied.");
  }

  async function revoke(invitation: Invitation): Promise<void> {
    if (!confirm("Revoke this invitation?")) return;
    await revokeInvitation(invitation.id);
    await load();
  }
</script>

<section class="page-layout">
  <div class="page-heading">
    <div><p class="eyebrow">Administration</p><h1>Invitations</h1></div>
    {#if $appState.config.invitations_enabled}<button class="button primary" onclick={create}><Icon name="plus"/> Create invitation</button>{/if}
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      {#if !$appState.config.invitations_enabled}
        <div class="notice">Invitation redemption is disabled in site settings. Existing invitations can still be reviewed or revoked.</div>
      {/if}
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else}
        <div class="panel data-list">
          {#each invitations as invitation (invitation.id)}
            <article class="data-row">
              <div>
                <strong><code>{invitation.token_prefix}…</code></strong>
                <small>{invitation.redeemed_by_username ? `Redeemed by ${invitation.redeemed_by_username}` : `${invitation.status} · expires ${formatDate(invitation.expires_at)}`}</small>
              </div>
              <div class="row-actions">
                {#if invitation.url}<button class="icon-button" title="Copy invitation" aria-label="Copy invitation" onclick={() => copy(invitation)}><Icon name="copy"/></button>{/if}
                {#if invitation.status === "active"}<button class="button danger" onclick={() => revoke(invitation)}>Revoke</button>{/if}
              </div>
            </article>
          {:else}
            <div class="empty"><p>No invitations.</p></div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
</section>
