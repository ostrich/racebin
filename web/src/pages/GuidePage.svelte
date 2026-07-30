<script lang="ts">
  const example = `curl -H "Authorization: Bearer $RACEBIN_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"title":"Example","content":"Hello","language":"plaintext","visibility":"unlisted"}' \\
  https://example.com/api/v1/pastes`;
</script>

<section>
  <div class="page-heading">
    <div><p class="eyebrow">Reference</p><h1>API guide</h1></div>
    <a class="button" href="/api/v1/openapi.json">OpenAPI JSON</a>
  </div>
  <section class="panel">
    <h2>Authentication</h2>
    <p>Use <code>Authorization: Bearer rbk_…</code>. Browser requests use the secure session cookie and send <code>X-CSRF-Token</code> for mutations.</p>
    <h2>Paste example</h2><pre class="content"><code>{example}</code></pre>
    <p>All errors use <code>{"{"}"error":{"{"}"code":"…","message":"…"{"}"}{"}"}</code>. Timestamps are Unix seconds.</p>
  </section>
</section>
