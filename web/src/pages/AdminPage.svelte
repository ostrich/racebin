<script lang="ts">
  import AdminNav from "../components/AdminNav.svelte";
  import Icon from "../components/Icon.svelte";
  import Link from "../components/Link.svelte";
  import { appState } from "../state";
</script>

<section class="stack page-stack">
  <div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Overview</h1><p class="muted">Manage content, accounts, and access from one workspace.</p></div></div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      <div class="admin-links">
        <Link href="/admin/pastes"><Icon name="file-text"/><div><strong>Pastes</strong><span>Review, filter, and remove stored content</span></div></Link>
        <Link href="/admin/users"><Icon name="user-round"/><div><strong>Users</strong><span>Manage account access and recovery</span></div></Link>
        <Link href="/admin/invitations"><Icon name="plus"/><div><strong>Invitations</strong><span>Create and revoke registration links</span></div></Link>
        <Link href="/admin/api-keys"><Icon name="key-round"/><div><strong>API keys</strong><span>Review administrative API access</span></div></Link>
        {#if $appState.session.user?.role === "owner"}
          <Link href="/admin/settings"><Icon name="sliders-horizontal"/><div><strong>Site settings</strong><span>Configure defaults and public features</span></div></Link>
          <Link href="/admin/audit"><Icon name="scroll-text"/><div><strong>Audit log</strong><span>Review sensitive administrative activity</span></div></Link>
        {/if}
      </div>
    </div>
  </div>
</section>
