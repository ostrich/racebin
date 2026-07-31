<script lang="ts">
  import { onMount, tick } from "svelte";
  import { requestApi } from "../api";
  import AttachmentList from "../components/AttachmentList.svelte";
  import CodeEditor from "../components/CodeEditor.svelte";
  import ConversionDialog from "../components/ConversionDialog.svelte";
  import LanguagePicker from "../components/LanguagePicker.svelte";
  import Link from "../components/Link.svelte";
  import { pasteDisplayTitle } from "../format";
  import { normalizeLanguage } from "../highlighting";
  import { showNotice } from "../notices";
  import { deferRouteReady, guardUnsavedChanges, navigate } from "../router";
  import { appState } from "../state";
  import type { Folder, FolderOverview, Paste, RichTextDocument } from "../types";

  type ContentKind = "text" | "rich_text";
  type Conversion = { content: string; document: RichTextDocument | null };

  let { pasteId }: { pasteId?: string } = $props();
  let paste = $state<Paste | null>(null);
  let loading = $state(false);
  let error = $state("");
  let files = $state<HTMLInputElement>();
  let conversionDialog: ConversionDialog;
  let title = $state("");
  let content = $state("");
  let document = $state<RichTextDocument>({ type: "doc", content: [{ type: "paragraph" }] });
  let contentKind = $state<ContentKind>("text");
  let folderId = $state("");
  let folders = $state<Folder[]>([]);
  let language = $state("auto");
  let visibility = $state("unlisted");
  let expiresAt = $state("");
  let readLimit = $state("");
  let attachmentSelection = $state("");
  let submitting = $state(false);
  let switching = $state(false);
  let baseline = $state("");
  let initialized = $state(false);
  const initialLoadReady = deferRouteReady();
  const drafts = new Map<ContentKind, string>();
  let canOrganize = $derived(!paste || paste.owner_id === $appState.session.user?.id);

  function snapshot(): string {
    return JSON.stringify({
      title, content, document: contentKind === "rich_text" ? document : null,
      contentKind, folderId, language: contentKind === "text" ? language : null,
      visibility, expiresAt, readLimit, attachmentSelection
    });
  }
  let dirty = $derived(initialized && snapshot() !== baseline);

  $effect(() => {
    guardUnsavedChanges(() => dirty);
    return () => guardUnsavedChanges();
  });

  function initialize(source?: Paste): void {
    paste = source ?? null;
    title = source?.title ?? "";
    content = source?.content ?? "";
    document = source?.document ?? { type: "doc", content: [{ type: "paragraph" }] };
    contentKind = source?.content_kind ?? "text";
    folderId = source?.folder_id ? String(source.folder_id) : (
      source ? "" : new URLSearchParams(location.search).get("folder_id") ?? ""
    );
    language = normalizeLanguage(source?.language ?? "auto") ?? "auto";
    visibility = source?.visibility ?? "unlisted";
    expiresAt = source?.expires_at
      ? new Date(source.expires_at * 1000).toISOString().slice(0, 16)
      : "";
    readLimit = source?.read_limit ? String(source.read_limit) : "";
    drafts.set(contentKind, content);
    void tick().then(() => {
      baseline = snapshot();
      initialized = true;
      initialLoadReady();
    });
  }

  onMount(() => {
    void requestApi<FolderOverview>("/folders")
      .then(result => { folders = result.items; })
      .catch(reason => showNotice(reason instanceof Error ? reason.message : "Unable to load folders", "error"));
    loading = Boolean(pasteId);
    if (!pasteId) {
      initialize();
      loading = false;
      return;
    }
    void requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}`)
      .then(initialize)
      .catch(reason => { error = reason instanceof Error ? reason.message : "Unable to load paste"; })
      .finally(() => {
        loading = false;
        initialLoadReady();
      });
  });

  async function convert(
    sourceKind: "text" | "rich_text",
    targetKind: "text" | "rich_text"
  ): Promise<Conversion> {
    return requestApi<Conversion>("/pastes/convert", {
      method: "POST",
      body: JSON.stringify({
        source_kind: sourceKind,
        target_kind: targetKind,
        content,
        document: sourceKind === "rich_text" ? document : null
      })
    });
  }

  async function changeKind(event: Event): Promise<void> {
    const selector = event.currentTarget as HTMLSelectElement;
    const source = contentKind;
    const target = selector.value as ContentKind;
    selector.value = source;
    if (source === target) return;
    switching = true;
    try {
      if (source === "rich_text") {
        const converted = await convert("rich_text", "text");
        if (converted.content && !(await conversionDialog.ask(target, converted.content))) return;
        drafts.set(target, converted.content);
        content = converted.content;
      } else if (target === "rich_text") {
        drafts.set(source, content);
        const converted = await convert("text", "rich_text");
        if (converted.content && !(await conversionDialog.ask(target, converted.content))) return;
        document = converted.document ?? { type: "doc", content: [{ type: "paragraph" }] };
      }
      contentKind = target;
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Conversion failed", "error");
    } finally {
      switching = false;
    }
  }

  function selectedFiles(): void {
    attachmentSelection = [...(files?.files ?? [])]
      .map(file => `${file.name}:${file.size}:${file.lastModified}`).join("|");
  }

  async function submit(): Promise<void> {
    const canonicalLanguage = contentKind === "text" ? normalizeLanguage(language) : "plaintext";
    if (!canonicalLanguage) {
      showNotice("Choose a supported language.", "error");
      return;
    }
    submitting = true;
    let created: Paste | undefined;
    try {
      const body = {
        title,
        content,
        document: contentKind === "rich_text" ? document : undefined,
        content_kind: contentKind,
        language: canonicalLanguage,
        visibility,
        expires_at: expiresAt ? Math.floor(new Date(expiresAt).getTime() / 1000) : null,
        read_limit: readLimit ? Number(readLimit) : null,
        ...(canOrganize ? { folder_id: folderId ? Number(folderId) : null } : {})
      };
      created = pasteId
        ? await requestApi<Paste>(`/pastes/${encodeURIComponent(pasteId)}`, {
            method: "PATCH", body: JSON.stringify(body)
          })
        : await requestApi<Paste>("/pastes", { method: "POST", body: JSON.stringify(body) });
      const selected = [...(files?.files ?? [])];
      if (selected.length) {
        const upload = new FormData();
        selected.forEach(file => upload.append("attachments", file));
        try {
          await requestApi(`/pastes/${encodeURIComponent(created.id)}/attachments`, {
            method: "POST", body: upload
          });
        } catch (reason) {
          if (!pasteId) {
            await requestApi(`/pastes/${encodeURIComponent(created.id)}`, { method: "DELETE" })
              .catch(() => undefined);
          }
          throw reason;
        }
      }
      initialized = false;
      guardUnsavedChanges();
      await navigate(`/pastes/${created.id}`);
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to save paste", "error");
    } finally {
      submitting = false;
    }
  }

  async function deletePaste(): Promise<void> {
    if (!pasteId || !confirm("Delete this paste permanently?")) return;
    try {
      await requestApi(`/pastes/${encodeURIComponent(pasteId)}`, { method: "DELETE" });
      initialized = false;
      guardUnsavedChanges();
      await navigate("/pastes");
    } catch (reason) {
      showNotice(reason instanceof Error ? reason.message : "Unable to delete paste", "error");
    }
  }
</script>

<ConversionDialog bind:this={conversionDialog}/>
{#if loading}
  <p class="muted">Loading paste…</p>
{:else if error}
  <section class="empty"><h1>Unable to edit this paste</h1><p>{error}</p><Link class="button" href="/pastes">Return to pastes</Link></section>
{:else}
  <section class="editor">
    <div class="page-heading"><div><p class="eyebrow">{paste ? "Edit" : "Create"}</p><h1>{paste ? pasteDisplayTitle(paste) : "New paste"}</h1></div></div>
    <form onsubmit={(event) => { event.preventDefault(); void submit(); }}>
      <label class="title-field"><span>Title</span><input bind:value={title} maxlength="200" placeholder="Optional title"/></label>
      {#if contentKind === "rich_text"}
        <div class="content-field"><span>Content</span>
          <div class="content-editor content-editor-rich">
            {#await import("../components/RichTextEditor.svelte") then module}
              {@const RichTextEditor = module.default}
              <RichTextEditor bind:document/>
            {/await}
          </div>
        </div>
      {:else}
        <div class="content-field"><span>Content</span>
          <div class="content-editor content-editor-text">
            <CodeEditor bind:value={content} bind:language/>
          </div>
        </div>
      {/if}
      <div class="form-grid">
        <label><span>Type</span><select value={contentKind} disabled={switching} onchange={changeKind}>
          <option value="text">Text</option><option value="rich_text">Rich text</option>
        </select></label>
        <LanguagePicker bind:value={language} disabled={contentKind !== "text"}/>
        {#if canOrganize}
          <label><span>Folder</span><select bind:value={folderId}>
            <option value="">Uncategorized</option>
            {#each folders as folder}<option value={String(folder.id)}>{folder.name}</option>{/each}
          </select></label>
        {/if}
        <label><span>Visibility</span><select bind:value={visibility}>
          <option value="public">public</option><option value="unlisted">unlisted</option><option value="private">private</option>
        </select></label>
        <label><span>Expires</span><input type="datetime-local" bind:value={expiresAt}/></label>
        <label><span>Read limit</span><input type="number" min="1" bind:value={readLimit} placeholder="Unlimited"/></label>
      </div>
      {#if paste?.attachments.length}
        <div class="existing-attachments"><span>Current attachments</span>
          <AttachmentList pasteId={paste.id} attachments={paste.attachments} canDelete editing
            ondelete={(attachment) => { if (paste) paste.attachments = paste.attachments.filter(item => item.id !== attachment.id); }}/>
          <small>Deleting an existing attachment takes effect immediately, even if you cancel editing.</small>
        </div>
      {/if}
      {#if $appState.config.attachments_enabled}
        <label><span>Add attachments</span><input bind:this={files} type="file" multiple onchange={selectedFiles}/>
          <small>Combined upload limit: {Math.floor($appState.config.max_attachment_size_bytes / 1024 / 1024)} MiB</small></label>
      {/if}
      <div class="actions">
        <button class="button primary" type="submit" disabled={submitting || switching}>{submitting ? "Saving…" : paste ? "Save changes" : "Create paste"}</button>
        <Link class="button" href={paste ? `/pastes/${paste.id}` : "/pastes"}>Cancel</Link>
        {#if paste}<button class="button danger" type="button" onclick={deletePaste}>Delete</button>{/if}
      </div>
    </form>
  </section>
{/if}
