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
        <p><Link href="/pastes/new">New</Link> creates a text or rich-text paste. <Link href="/pastes">My pastes</Link> lets you search, organize, move, and edit what you have saved. Public pastes appear in Explore; unlisted pastes are available only to people with the link; private pastes require your account.</p>
      </section>
    </div>
  </div>
</section>
