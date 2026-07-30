import { api } from "../api";
import { navigate } from "../router";
import { state } from "../state";
import type { Page, Paste } from "../types";
import { date, esc, icon, layout, title } from "../ui";

export async function home(): Promise<void> {
  if (state.session.user) return pasteForm();
  const page = await api<Page<Paste>>("/pastes?access=public&page_size=8");
  layout(`
    <section class="welcome"><div><p class="eyebrow">Share text and files</p><h1>${esc(state.config.name)}</h1><p>Public pastes are open to everyone. Sign in to create and manage your own.</p>
    <div class="actions"><a class="button primary" href="/explore" data-link>Explore pastes</a><a class="button" href="/login" data-link>Log in</a></div></div></section>
    <section><div class="section-heading"><h2>Recently shared</h2><a href="/explore" data-link>View all</a></div>${pasteRows(page.items)}</section>`);
}

export function pasteRows(items: Paste[], manage = false): string {
  if (!items.length) return `<div class="empty compact"><p>No pastes found.</p></div>`;
  return `<div class="paste-list">${items.map(paste => `
    <article class="paste-row">
      <div class="paste-main"><a class="paste-title" href="/pastes/${esc(paste.slug)}" data-link>${esc(title(paste))}</a>
      <p>${esc(paste.content.slice(0, 160).replace(/\s+/g, " "))}</p></div>
      <div class="paste-meta"><span>${esc(paste.syntax)}</span><span>${esc(paste.access)}</span><time>${date(paste.created)}</time></div>
      <div class="row-actions">
        ${icon("copy", "Copy link")}
        ${manage ? `<a class="icon-button" title="Edit" aria-label="Edit" href="/pastes/${esc(paste.slug)}/edit" data-link><i data-icon="edit-3"></i></a>${icon("trash-2", "Delete")}` : ""}
      </div><input type="hidden" value="${esc(paste.slug)}">
    </article>`).join("")}</div>`;
}

export function pagination(page: Page<unknown>): string {
  const pages = Math.max(1, Math.ceil(page.total / page.page_size));
  if (pages === 1) return "";
  const link = (number: number, label: string) => {
    const params = new URLSearchParams(location.search);
    params.set("page", String(number));
    return `<a class="button" href="${location.pathname}?${params}" data-link>${label}</a>`;
  };
  return `<nav class="pagination" aria-label="Pagination">
    ${page.page > 1 ? link(page.page - 1, "Previous") : ""}
    <span>Page ${page.page} of ${pages}</span>
    ${page.page < pages ? link(page.page + 1, "Next") : ""}
  </nav>`;
}

export async function pasteList(mine: boolean): Promise<void> {
  if (mine && !state.session.user) return navigate("/login");
  const params = new URLSearchParams(location.search);
  params.set("page_size", "50");
  if (mine) params.set("mine", "true");
  if (!mine) params.set("access", "public");
  const page = await api<Page<Paste>>(`/pastes?${params}`);
  layout(`<section>
    <div class="page-heading"><div><p class="eyebrow">${mine ? "Workspace" : "Public"}</p><h1>${mine ? "My pastes" : "Explore"}</h1></div>${mine ? `<a class="button primary" href="/new" data-link><i data-icon="plus"></i> New paste</a>` : ""}</div>
    <form class="filters" id="paste-filters"><label><span>Search</span><input name="search" value="${esc(params.get("search") ?? "")}" placeholder="Title, content, or ID"></label>
    <label><span>Access</span><select name="access"><option value="">All access</option>${["public","unlisted","owner"].map(v => `<option ${params.get("access") === v ? "selected" : ""}>${v}</option>`).join("")}</select></label>
    <button class="button" type="submit"><i data-icon="search"></i> Filter</button></form>
    <p class="result-count">${page.total} paste${page.total === 1 ? "" : "s"}</p>${pasteRows(page.items, mine)}${pagination(page)}</section>`);
}

export async function pasteView(slug: string): Promise<void> {
  const paste = await api<Paste>(`/pastes/${encodeURIComponent(slug)}/consume`);
  const own = state.session.user && (state.session.user.id === paste.owner_user_id || state.session.user.role === "admin");
  if (paste.kind === "url") {
    layout(`<section class="empty"><p class="eyebrow">Short link</p><h1>${esc(title(paste))}</h1><p>Redirecting…</p></section>`);
    location.replace(paste.content);
    return;
  }
  layout(`<article class="paste-view">
    <div class="page-heading"><div><p class="eyebrow">${esc(paste.access)} · ${esc(paste.syntax)}</p><h1>${esc(title(paste))}</h1></div>
    <div class="actions"><a class="button" href="/api/v2/pastes/${esc(paste.slug)}/raw">Raw</a>${paste.files.length ? `<a class="button" href="/api/v2/pastes/${esc(paste.slug)}/archive">ZIP</a>` : ""}${state.config.qr ? `<a class="button" href="/api/v2/pastes/${esc(paste.slug)}/qr">QR</a>` : ""}${own ? `<a class="button primary" href="/pastes/${esc(paste.slug)}/edit" data-link><i data-icon="edit-3"></i> Edit</a>` : ""}</div></div>
    <pre class="content"><code>${esc(paste.content)}</code></pre>
    ${paste.files.length ? `<section><h2>Files</h2><div class="files">${paste.files.map(file => `<div class="file-row" data-file-id="${file.id}" data-slug="${esc(paste.slug)}"><a href="/api/v2/pastes/${esc(paste.slug)}/files/${file.id}"><i data-icon="file-text"></i><span>${esc(file.name)}</span><small>${file.size.toLocaleString()} bytes</small></a>${own ? `<button class="icon-button" type="button" title="Delete file" aria-label="Delete file" data-action="delete-file"><i data-icon="trash-2"></i></button>` : ""}</div>`).join("")}</div></section>` : ""}
    <footer class="paste-stats"><span>Created ${date(paste.created)}</span><span>Expires ${date(paste.expiration)}</span><span>${paste.read_count} reads</span></footer>
  </article>`);
}

export async function pasteForm(slug?: string): Promise<void> {
  if (!state.session.user) return navigate("/login");
  const paste = slug ? await api<Paste>(`/pastes/${encodeURIComponent(slug)}`) : undefined;
  layout(`<section class="editor">
    <div class="page-heading"><div><p class="eyebrow">${paste ? "Edit" : "Create"}</p><h1>${paste ? esc(title(paste)) : "New paste"}</h1></div></div>
    <form id="paste-form">
      <label class="title-field"><span>Title</span><input name="title" maxlength="200" value="${esc(paste?.title ?? "")}" placeholder="Optional title"></label>
      <label><span>Content</span><textarea name="content" spellcheck="false">${esc(paste?.content ?? "")}</textarea></label>
      <div class="form-grid">
        <label><span>Type</span><select name="kind"><option value="text">Text</option><option value="url" ${paste?.kind === "url" ? "selected" : ""}>URL</option></select></label>
        <label><span>Syntax</span><select name="syntax">${["none","auto","sh","c","cpp","cs","go","html","java","js","json","kt","lua","php","py","r","rb","rs","swift","xml","yaml"].map(v => `<option ${paste?.syntax === v ? "selected" : ""}>${v}</option>`).join("")}</select></label>
        <label><span>Access</span><select name="access">${["public","unlisted","owner"].map(v => `<option ${paste?.access === v || (!paste && v === "unlisted") ? "selected" : ""}>${v}</option>`).join("")}</select></label>
        <label><span>Expires</span><input type="datetime-local" name="expiration" value="${paste?.expiration ? new Date(paste.expiration * 1000).toISOString().slice(0,16) : ""}"></label>
        <label><span>Burn after reads</span><input type="number" min="0" name="burn_after_reads" value="${paste?.burn_after_reads ?? 0}"></label>
      </div>
      ${state.config.file_uploads ? `<label><span>Add files</span><input type="file" name="files" multiple><small>Combined upload limit: ${Math.floor(state.config.max_file_size / 1024 / 1024)} MiB</small></label>` : ""}
      <div class="actions"><button class="button primary" type="submit">${paste ? "Save changes" : "Create paste"}</button>${paste ? `<button class="button danger" type="button" data-action="delete-paste">Delete</button>` : ""}</div>
      <input type="hidden" name="slug" value="${esc(slug ?? "")}">
    </form></section>`);
}
