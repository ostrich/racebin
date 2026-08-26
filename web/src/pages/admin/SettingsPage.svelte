<script lang="ts">
  import { onMount } from "svelte";
  import {
    getInstanceSettings,
    replaceInstanceSettings,
    type InstanceSettings
  } from "../../api";
  import AdminNav from "../../components/AdminNav.svelte";
  import { availableLanguageOptions } from "../../highlighting";
  import { holdNavigation } from "../../navigation";
  import { showNotice } from "../../app/notices";
  import { loadCapabilities } from "../../app/session";
  import { appState } from "../../app/state";

  let settings = $state<InstanceSettings | null>(null);
  let error = $state("");
  let saving = $state(false);
  let languageOptions = $derived(availableLanguageOptions($appState.languages));
  const initialLoadReady = holdNavigation();

  onMount(() => {
    void getInstanceSettings()
      .then(value => { settings = value; })
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load settings"; })
      .finally(initialLoadReady);
  });

  async function save(): Promise<void> {
    if (!settings) return;
    saving = true;
    try {
      settings = await replaceInstanceSettings(settings);
      await loadCapabilities();
      showNotice("Settings saved.");
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to save settings", "error");
    } finally {
      saving = false;
    }
  }

  function changeExpiration(event: Event): void {
    if (!settings) return;
    const value = (event.currentTarget as HTMLSelectElement).value;
    settings.default_expiration_seconds = value ? Number(value) : null;
  }
</script>

<section class="page-layout">
  <div class="page-heading">
    <div>
      <p class="eyebrow">Owner</p>
      <h1>Site settings</h1>
    </div>
    <div class="page-heading-actions"><button class="button primary" disabled={saving || !settings} onclick={save}>Save settings</button></div>
  </div>
  <div class="section-layout">
    <AdminNav/>
    <div class="section-content">
      {#if error}
        <section class="empty"><p>{error}</p></section>
      {:else if settings}
        <form class="stack" onsubmit={(event) => { event.preventDefault(); void save(); }}>
          <section class="panel settings-section">
            <div><h2>Identity and access</h2><p class="muted">Control how the site presents itself and how new users join.</p></div>
            <div class="settings-grid">
              <label class="field"><span>Site name</span><input bind:value={settings.site_name} maxlength="64"></label>
              <label class="field"><span>Home page</span><select bind:value={settings.home_mode}><option value="standard">Standard introduction</option><option value="plain">Login-focused</option></select></label>
              <label class="check"><input type="checkbox" bind:checked={settings.public_explore_enabled}><span>Show public Explore</span></label>
              <label class="check"><input type="checkbox" bind:checked={settings.invitations_enabled}><span>Allow invitation redemption</span></label>
            </div>
          </section>
          <section class="panel settings-section">
            <div><h2>Features</h2><p class="muted">Disable features without removing existing data.</p></div>
            <div class="settings-grid">
              <label class="check"><input type="checkbox" bind:checked={settings.attachments_enabled}><span>Allow attachments</span></label>
              <label class="check"><input type="checkbox" bind:checked={settings.qr_codes_enabled}><span>Offer QR codes</span></label>
            </div>
          </section>
          <section class="panel settings-section">
            <div><h2>New-paste defaults</h2><p class="muted">These values prefill new pastes; users can still change them.</p></div>
            <div class="settings-grid">
              <label class="field"><span>Format</span><select bind:value={settings.default_format}><option value="text">Text</option><option value="markdown">Rich text</option></select></label>
              <label class="field"><span>Language</span><select bind:value={settings.default_language}>{#each languageOptions as language}<option value={language.id}>{language.label}</option>{/each}</select></label>
              <label class="field"><span>Visibility</span><select bind:value={settings.default_visibility}><option value="public">Public</option><option value="unlisted">Unlisted</option><option value="private">Private</option></select></label>
              <label class="field"><span>Expiration</span><select value={settings.default_expiration_seconds ?? ""} onchange={changeExpiration}><option value="">Never</option><option value="3600">1 hour</option><option value="43200">12 hours</option><option value="86400">24 hours</option><option value="604800">1 week</option><option value="2592000">1 month</option><option value="31536000">1 year</option></select></label>
            </div>
          </section>
        </form>
      {/if}
    </div>
  </div>
</section>
