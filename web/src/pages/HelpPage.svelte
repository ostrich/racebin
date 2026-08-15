<script lang="ts">
  import Link from "../components/Link.svelte";
  import { showNotice } from "../notices";
  import { appState } from "../state";

  let apiBase = $derived($appState.config.api_base_url ?? `${location.origin}/api/v1`);
  const command = (...lines: string[]) => lines.join("\n");
  let examples = $derived([
    ["Create a paste", command(
      `curl -X POST "${apiBase}/pastes" \\`,
      `  -H "Authorization: Bearer $RACEBIN_API_KEY" \\`,
      `  -H "Content-Type: application/json" \\`,
      `  -d '{"title":"Example","body":{"format":"text","content":"Hello","language":"plaintext"},"visibility":"unlisted"}'`
    )],
    ["List your pastes", command(`curl "${apiBase}/pastes?owner=me" \\`, `  -H "Authorization: Bearer $RACEBIN_API_KEY"`)],
    ["Read a paste", command(`curl -X POST "${apiBase}/pastes/PASTE_ID/reads" \\`, `  -H "Authorization: Bearer $RACEBIN_API_KEY"`, `  -H "Idempotency-Key: $(uuidgen)"`)],
    ["Update a paste", command(
      `curl -X PATCH "${apiBase}/pastes/PASTE_ID" \\`,
      `  -H "Authorization: Bearer $RACEBIN_API_KEY" \\`,
      `  -H "If-Match: *" \\`,
      `  -H "Content-Type: application/json" \\`,
      `  -d '{"title":"Updated title","visibility":"public"}'`
    )],
    ["Read plain text", command(`curl -X POST "${apiBase}/pastes/PASTE_ID/reads" \\`, `  -H "Authorization: Bearer $RACEBIN_API_KEY"`, `  -H "Accept: text/plain"`)],
    ["Upload an attachment", command(
      `curl -X POST "${apiBase}/pastes/PASTE_ID/attachments" \\`,
      `  -H "Authorization: Bearer $RACEBIN_API_KEY" \\`,
      `  -H "If-Match: *" \\`,
      `  -F "file=@./example.txt"`
    )],
    ["Delete a paste", command(`curl -X DELETE "${apiBase}/pastes/PASTE_ID" \\`, `  -H "Authorization: Bearer $RACEBIN_API_KEY" \\`, `  -H "If-Match: *"`)]
  ] as const);

  async function copy(value: string): Promise<void> {
    await navigator.clipboard.writeText(value);
    showNotice("Command copied.");
  }
</script>

<section class="help-page">
  <div class="page-heading">
    <div><p class="eyebrow">Help</p><h1>Using Racebin</h1></div>
    <a class="button" href={`${apiBase}/openapi.json`}>OpenAPI JSON</a>
  </div>
  <div class="help-layout">
    <aside class="panel help-index sticky-sidebar" aria-label="Help topics">
      <Link href="#api-keys">API keys</Link><Link href="#examples">Examples</Link><Link href="#scopes">Scopes</Link><Link href="#basics">Site basics</Link>
    </aside>
    <div class="help-content">
      <section class="panel" id="api-keys">
        <h2>API keys</h2>
        <p>Create a key under <Link href="/account">Account</Link>, choose only the privileges your tool needs, and copy it when it is shown. Racebin cannot display the full key again.</p>
        <p>Store it in an environment variable instead of putting it directly in a script:</p>
        <pre><code>export RACEBIN_API_KEY='rbk_…'</code></pre>
        <p>Send the key with every API request as <code>Authorization: Bearer $RACEBIN_API_KEY</code>. Treat it like a password and revoke it from your account if it is exposed.</p>
      </section>
      <section class="panel" id="examples">
        <h2>Command examples</h2>
        <p>These commands use this Racebin installation automatically.</p>
        <div class="help-examples">
          {#each examples as [title, value]}
            <article><div><h3>{title}</h3><button class="button" type="button" onclick={() => copy(value)}>Copy</button></div><pre><code>{value}</code></pre></article>
          {/each}
        </div>
      </section>
      <section class="panel" id="scopes">
        <h2>Key privileges</h2>
        <dl class="scope-list">
          {#each $appState.config.scopes as scope}
            <div><dt><code>{scope.id}</code></dt><dd>{scope.description}</dd></div>
          {/each}
        </dl>
      </section>
      <section class="panel" id="basics">
        <h2>Site basics</h2>
        <p>Racebin keeps text, formatted documents, and attachments together under one shareable link. You need an account to create and manage pastes, but the people you share with usually do not.</p>

        <div class="help-basics">
          <section>
            <h3>Create a paste</h3>
            <ol>
              <li>Open <Link href="/pastes/new">New paste</Link> and add an optional title.</li>
              <li>Choose <strong>Text</strong> for code and plain text, or <strong>Rich text</strong> for a visual editor backed by portable Markdown. Rich text can switch between Visual and Markdown modes.</li>
              <li>For text, leave the language as <strong>Plaintext</strong> for no highlighting, choose <strong>Auto</strong> to detect it, or select a language yourself.</li>
              <li>Choose who can open it, and optionally set a folder, expiration, view limit, or attachments.</li>
              <li>Select <strong>Create paste</strong>, then copy its page URL to share it.</li>
            </ol>
          </section>

          <section>
            <h3>Choose the right visibility</h3>
            <dl class="help-definitions">
              <div><dt>Public</dt><dd>Anyone can open it, and it can appear in <Link href="/explore">Explore</Link>.</dd></div>
              <div><dt>Unlisted</dt><dd>Anyone with the URL can open it, but it does not appear in Explore. The URL is access, not a password.</dd></div>
              <div><dt>Private</dt><dd>Only you can open it through the site. Site administrators can still manage stored content.</dd></div>
            </dl>
          </section>

          <section>
            <h3>Control how long it remains available</h3>
            <p><strong>Expiration</strong> removes a paste after the selected date and time. <strong>View limit</strong> removes it after the specified number of views. Leave either setting at its default for no limit.</p>
            <p>When you are signed in and open your own paste in the browser, that visit does not increase its view count or consume a limited view.</p>
          </section>

          <section>
            <h3>View and share</h3>
            <ul>
              <li><strong>Copy</strong> copies the paste content; <strong>Raw</strong> opens it without the page interface.</li>
              <li>Long code displays a horizontal scrollbar. Use <strong>Wrap</strong> when it appears to fit long lines to the viewer.</li>
              {#if $appState.config.attachments_enabled}<li>Attachments can be downloaded separately or together with the paste as a ZIP archive.</li>{/if}
              {#if $appState.config.qr_codes_enabled}<li><strong>QR</strong> creates a scannable link to the paste.</li>{/if}
            </ul>
          </section>

          <section>
            <h3>Organize and manage</h3>
            <p><Link href="/pastes">My pastes</Link> is your working library. Search its contents and metadata, filter and sort the list, switch between normal and compact views, and use folders to group related items.</p>
            <p>Select one or more pastes to move them together. Open a paste to edit or delete it. Deleting an existing attachment while editing takes effect immediately, even if you later cancel the rest of the edit.</p>
          </section>

          <aside class="help-note">
            <h3>Before sharing sensitive information</h3>
            <p>Check the visibility, expiration, and attached files. Remove passwords, private keys, access tokens, and other secrets; an unlisted URL can still be forwarded to someone else.</p>
          </aside>
        </div>
      </section>
    </div>
  </div>
</section>
