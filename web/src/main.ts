import "./style.css";
import { api } from "./api";
import { renderIcons } from "./icons";
import { state } from "./state";
import type { ApiKey, Config, Page, Paste, Session } from "./types";

const app = document.querySelector<HTMLDivElement>("#app")!;
let session = state.session;
let config = state.config;

const esc = (value: unknown) => String(value ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]!));
const date = (value: number | null) => value ? new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(value * 1000) : "Never";
const title = (paste: Paste) => paste.title || paste.slug;
const icon = (name: string, label: string) => `<button class="icon-button" type="button" title="${esc(label)}" aria-label="${esc(label)}" data-action="${name}"><i data-icon="${name}"></i></button>`;
const formValue = (form: FormData, key: string) => String(form.get(key) ?? "");

function layout(content: string): void {
  const user = session.user;
  app.innerHTML = `
    <header>
      <a class="brand" href="/" data-link>${esc(config.name)}</a>
      <nav>
        <a href="/explore" data-link>Explore</a>
        ${user ? `<a href="/pastes" data-link>My pastes</a><a href="/new" data-link><i data-icon="plus"></i> New</a>` : ""}
        ${user?.role === "admin" ? `<a href="/admin" data-link>Admin</a>` : ""}
      </nav>
      <div class="session">
        ${user ? `<a href="/account" data-link><i data-icon="user-round"></i><span>${esc(user.username)}</span></a>${icon("log-out", "Log out")}` :
          `<a href="/login" data-link><i data-icon="log-in"></i><span>Log in</span></a>`}
      </div>
    </header>
    <main>${content}</main>
    <div id="toast" role="status" aria-live="polite"></div>`;
  renderIcons();
}

function notice(message: string, kind = ""): void {
  const toast = document.querySelector<HTMLDivElement>("#toast");
  if (!toast) return;
  toast.textContent = message;
  toast.className = `show ${kind}`;
  window.setTimeout(() => toast.className = "", 3500);
}

async function loadSession(): Promise<void> {
  [session, config] = await Promise.all([
    api<Session>("/session").catch(() => ({ authenticated: false })),
    api<Config>("/config")
  ]);
  state.session = session;
  state.config = config;
  if (session.user?.force_password_change && location.pathname !== "/account/password") {
    history.replaceState({}, "", "/account/password");
  }
}

function navigate(path: string): void {
  history.pushState({}, "", path);
  void route();
}

function errorView(error: unknown): void {
  const message = error instanceof Error ? error.message : "The request failed.";
  layout(`<section class="empty"><h1>Unable to load this page</h1><p>${esc(message)}</p><a class="button" href="/" data-link>Return home</a></section>`);
}

async function route(): Promise<void> {
  const path = location.pathname;
  try {
    if (path === "/") return await home();
    if (path === "/explore") return await pasteList(false);
    if (path === "/login") return loginView();
    if (path === "/new") return pasteForm();
    if (path === "/pastes") return await pasteList(true);
    if (path === "/account") return await accountView();
    if (path === "/account/password") return passwordView();
    if (path === "/admin") return adminView();
    if (path === "/admin/pastes") return await adminPastes();
    if (path === "/guide") return guideView();
    if (path.startsWith("/invite/")) return inviteView(path.slice(8));
    const edit = path.match(/^\/pastes\/([^/]+)\/edit$/);
    if (edit?.[1]) return await pasteForm(edit[1]);
    const view = path.match(/^\/pastes\/([^/]+)$/);
    if (view?.[1]) return await pasteView(view[1]);
    layout(`<section class="empty"><h1>Page not found</h1><p>The requested page does not exist.</p></section>`);
  } catch (error) { errorView(error); }
}

function guideView(): void {
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Reference</p><h1>API guide</h1></div><a class="button" href="/api/v2/openapi.json">OpenAPI JSON</a></div>
    <section class="panel"><h2>Authentication</h2><p>Use <code>Authorization: Bearer rbk_…</code>. Browser requests use the secure session cookie and send <code>X-CSRF-Token</code> for mutations.</p>
    <h2>Paste example</h2><pre class="content"><code>curl -H "Authorization: Bearer $RACEBIN_KEY" \\
  -H "Content-Type: application/json" \\
  -d '{"title":"Example","content":"Hello","syntax":"none","access":"unlisted"}' \\
  https://example.com/api/v2/pastes</code></pre>
    <p>All errors use <code>{"error":{"code":"…","message":"…"}}</code>. Timestamps are Unix seconds.</p></section></section>`);
}

async function home(): Promise<void> {
  if (session.user) return pasteForm();
  const page = await api<Page<Paste>>("/pastes?access=public&page_size=8");
  layout(`
    <section class="welcome"><div><p class="eyebrow">Share text and files</p><h1>${esc(config.name)}</h1><p>Public pastes are open to everyone. Sign in to create and manage your own.</p>
    <div class="actions"><a class="button primary" href="/explore" data-link>Explore pastes</a><a class="button" href="/login" data-link>Log in</a></div></div></section>
    <section><div class="section-heading"><h2>Recently shared</h2><a href="/explore" data-link>View all</a></div>${pasteRows(page.items)}</section>`);
}

function pasteRows(items: Paste[], manage = false): string {
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

function pagination(page: Page<unknown>): string {
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

async function pasteList(mine: boolean): Promise<void> {
  if (mine && !session.user) return navigate("/login");
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

async function pasteView(slug: string): Promise<void> {
  const paste = await api<Paste>(`/pastes/${encodeURIComponent(slug)}/consume`);
  const own = session.user && (session.user.id === paste.owner_user_id || session.user.role === "admin");
  if (paste.kind === "url") {
    layout(`<section class="empty"><p class="eyebrow">Short link</p><h1>${esc(title(paste))}</h1><p>Redirecting…</p></section>`);
    location.replace(paste.content);
    return;
  }
  layout(`<article class="paste-view">
    <div class="page-heading"><div><p class="eyebrow">${esc(paste.access)} · ${esc(paste.syntax)}</p><h1>${esc(title(paste))}</h1></div>
    <div class="actions"><a class="button" href="/api/v2/pastes/${esc(paste.slug)}/raw">Raw</a>${paste.files.length ? `<a class="button" href="/api/v2/pastes/${esc(paste.slug)}/archive">ZIP</a>` : ""}${config.qr ? `<a class="button" href="/api/v2/pastes/${esc(paste.slug)}/qr">QR</a>` : ""}${own ? `<a class="button primary" href="/pastes/${esc(paste.slug)}/edit" data-link><i data-icon="edit-3"></i> Edit</a>` : ""}</div></div>
    <pre class="content"><code>${esc(paste.content)}</code></pre>
    ${paste.files.length ? `<section><h2>Files</h2><div class="files">${paste.files.map(file => `<div class="file-row" data-file-id="${file.id}" data-slug="${esc(paste.slug)}"><a href="/api/v2/pastes/${esc(paste.slug)}/files/${file.id}"><i data-icon="file-text"></i><span>${esc(file.name)}</span><small>${file.size.toLocaleString()} bytes</small></a>${own ? `<button class="icon-button" type="button" title="Delete file" aria-label="Delete file" data-action="delete-file"><i data-icon="trash-2"></i></button>` : ""}</div>`).join("")}</div></section>` : ""}
    <footer class="paste-stats"><span>Created ${date(paste.created)}</span><span>Expires ${date(paste.expiration)}</span><span>${paste.read_count} reads</span></footer>
  </article>`);
}

async function pasteForm(slug?: string): Promise<void> {
  if (!session.user) return navigate("/login");
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
      ${config.file_uploads ? `<label><span>Add files</span><input type="file" name="files" multiple><small>Combined upload limit: ${Math.floor(config.max_file_size / 1024 / 1024)} MiB</small></label>` : ""}
      <div class="actions"><button class="button primary" type="submit">${paste ? "Save changes" : "Create paste"}</button>${paste ? `<button class="button danger" type="button" data-action="delete-paste">Delete</button>` : ""}</div>
      <input type="hidden" name="slug" value="${esc(slug ?? "")}">
    </form></section>`);
}

function loginView(): void {
  layout(`<section class="auth"><form id="login-form"><p class="eyebrow">Account</p><h1>Log in</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required autofocus></label>
    <label><span>Password</span><input type="password" name="password" autocomplete="current-password" required></label>
    <label class="check"><input type="checkbox" name="remember"><span>Keep me signed in</span></label>
    <button class="button primary" type="submit">Log in</button></form></section>`);
}

function inviteView(token: string): void {
  layout(`<section class="auth"><form id="invite-form"><p class="eyebrow">Invitation</p><h1>Create your account</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required></label>
    <label><span>Password</span><input type="password" name="password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Create account</button><input type="hidden" name="token" value="${esc(token)}"></form></section>`);
}

async function accountView(): Promise<void> {
  if (!session.user) return navigate("/login");
  const keys = await api<ApiKey[]>("/account/api-keys");
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Settings</p><h1>Account</h1></div><a class="button" href="/account/password" data-link>Change password</a></div>
    <section class="panel"><h2>API keys</h2><p class="muted">Tokens are shown once when created.</p>
      <div class="key-list">${keys.length ? keys.map(key => `<div class="key-row"><div><strong>${esc(key.name)}</strong><code>rbk_${esc(key.prefix)}_...</code><small>${esc(key.scopes)}</small></div><label class="switch"><input type="checkbox" data-key="${key.id}" ${key.enabled ? "checked" : ""}><span></span></label>${icon("trash-2", "Delete API key")}<input type="hidden" value="${key.id}"></div>`).join("") : `<p class="empty compact">No API keys.</p>`}</div>
      <form id="key-form" class="key-form"><label><span>Name</span><input name="name" required maxlength="100"></label>
      <fieldset><legend>Scopes</legend>${["paste:read","paste:write","paste:delete","paste:list"].map(v => `<label class="check"><input type="checkbox" name="scopes" value="${v}"><span>${v}</span></label>`).join("")}</fieldset>
      <button class="button primary" type="submit"><i data-icon="key-round"></i> Create key</button></form>
    </section></section>`);
}

function passwordView(): void {
  if (!session.user) return navigate("/login");
  layout(`<section class="auth"><form id="password-form"><p class="eyebrow">Security</p><h1>Change password</h1>
    <label><span>Current password</span><input type="password" name="current_password" autocomplete="current-password" required></label>
    <label><span>New password</span><input type="password" name="new_password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Update password</button></form></section>`);
}

function adminView(): void {
  if (session.user?.role !== "admin") return navigate("/");
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>Admin</h1></div></div>
    <div class="admin-links"><a href="/admin/pastes" data-link><i data-icon="file-text"></i><div><strong>All pastes</strong><span>Search, filter, edit, and remove pastes</span></div></a>
    <button type="button" data-action="admin-users"><i data-icon="user-round"></i><div><strong>Users</strong><span>Roles and account access</span></div></button>
    <button type="button" data-action="admin-invites"><i data-icon="plus"></i><div><strong>Invitations</strong><span>Create and revoke invitation links</span></div></button>
    <button type="button" data-action="admin-keys"><i data-icon="key-round"></i><div><strong>API keys</strong><span>Review and revoke keys</span></div></button></div>
    <section id="admin-detail" class="panel hidden"></section></section>`);
}

async function adminPastes(): Promise<void> {
  if (session.user?.role !== "admin") return navigate("/");
  const params = new URLSearchParams(location.search); params.set("page_size", "100");
  const page = await api<Page<Paste>>(`/admin/pastes?${params}`);
  layout(`<section><div class="page-heading"><div><p class="eyebrow">Administration</p><h1>All pastes</h1></div><a class="button" href="/admin" data-link>Admin home</a></div>
    <form class="filters admin-filters" id="paste-filters"><label><span>Search</span><input name="search" value="${esc(params.get("search") ?? "")}"></label>
    <label><span>Access</span><select name="access"><option value="">All</option>${["public","unlisted","owner"].map(v=>`<option ${params.get("access")===v?"selected":""}>${v}</option>`).join("")}</select></label>
    <label><span>Owner ID</span><input type="number" name="owner_user_id" value="${esc(params.get("owner_user_id") ?? "")}"></label><button class="button" type="submit">Filter</button></form>
    <p class="result-count">${page.total} pastes</p>${pasteRows(page.items, true)}${pagination(page)}</section>`);
}

document.addEventListener("click", async event => {
  const target = event.target as HTMLElement;
  const link = target.closest<HTMLAnchorElement>("a[data-link]");
  if (link) { event.preventDefault(); navigate(link.pathname + link.search); return; }
  const action = target.closest<HTMLElement>("[data-action]")?.dataset.action;
  try {
    if (action === "log-out") { await api("/session", { method: "DELETE" }); session = state.session = { authenticated: false }; navigate("/"); }
    if (action === "copy") {
      const slug = target.closest<HTMLElement>(".paste-row")?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (slug) { await navigator.clipboard.writeText(`${location.origin}/pastes/${slug}`); notice("Link copied."); }
    }
    if (action === "trash-2") {
      const row = target.closest<HTMLElement>(".paste-row");
      const slug = row?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      const keyRow = target.closest<HTMLElement>(".key-row");
      const key = keyRow?.querySelector<HTMLInputElement>('input[type="hidden"]')?.value;
      if (slug && confirm("Delete this paste permanently?")) { await api(`/pastes/${slug}`, { method: "DELETE" }); row?.remove(); }
      if (key && confirm("Delete this API key permanently?")) { await api(`/account/api-keys/${key}`, { method: "DELETE" }); keyRow?.remove(); }
    }
    if (action === "delete-paste") {
      const slug = (document.querySelector<HTMLInputElement>('input[name="slug"]'))?.value;
      if (slug && confirm("Delete this paste permanently?")) { await api(`/pastes/${slug}`, { method: "DELETE" }); navigate("/pastes"); }
    }
    if (action === "delete-file") {
      const row = target.closest<HTMLElement>(".file-row");
      const slug = row?.dataset.slug;
      const fileId = row?.dataset.fileId;
      if (slug && fileId && confirm("Delete this file permanently?")) {
        await api(`/pastes/${encodeURIComponent(slug)}/files/${fileId}`, { method: "DELETE" });
        row.remove();
      }
    }
    if (action === "create-invite") {
      const invite = await api<{url:string}>("/admin/invites", { method: "POST" });
      await navigator.clipboard.writeText(`${location.origin}${invite.url}`);
      notice("Invitation link copied.");
      await loadAdmin("invites");
    }
    if (action === "revoke-invite") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id) { await api(`/admin/invites/${id}`, { method: "DELETE" }); await loadAdmin("invites"); }
    }
    if (action === "delete-admin-key") {
      const id = target.closest<HTMLElement>("[data-id]")?.dataset.id;
      if (id && confirm("Delete this API key permanently?")) { await api(`/admin/api-keys/${id}`, { method: "DELETE" }); await loadAdmin("keys"); }
    }
    if (action?.startsWith("admin-")) await loadAdmin(action.slice(6));
  } catch (error) { notice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("change", async event => {
  const input = event.target as HTMLInputElement;
  if (input.dataset.userEnabled) {
    try { await api(`/admin/users/${input.dataset.userEnabled}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.adminKey) {
    try { await api(`/admin/api-keys/${input.dataset.adminKey}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
    catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
    return;
  }
  if (input.dataset.userRole) {
    try { await api(`/admin/users/${input.dataset.userRole}`, { method: "PATCH", body: JSON.stringify({ role: input.value }) }); }
    catch (error) { notice(error instanceof Error ? error.message : "Request failed", "error"); await loadAdmin("users"); }
    return;
  }
  if (!input.dataset.key) return;
  try { await api(`/account/api-keys/${input.dataset.key}`, { method: "PATCH", body: JSON.stringify({ enabled: input.checked }) }); }
  catch (error) { input.checked = !input.checked; notice(error instanceof Error ? error.message : "Request failed", "error"); }
});

document.addEventListener("submit", async event => {
  event.preventDefault();
  const form = event.target as HTMLFormElement;
  const data = new FormData(form);
  const controls = [...form.querySelectorAll<HTMLButtonElement | HTMLInputElement>("button, input[type=submit]")];
  controls.forEach(control => control.disabled = true);
  try {
    if (form.id === "login-form") {
      await api("/session", { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password"), remember: data.has("remember") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "invite-form") {
      await api(`/invites/${encodeURIComponent(formValue(data,"token"))}/accept`, { method: "POST", body: JSON.stringify({ username: formValue(data,"username"), password: formValue(data,"password") }) });
      await loadSession(); navigate("/pastes");
    }
    if (form.id === "paste-form") {
      const expiration = formValue(data,"expiration");
      const body = {
        title: formValue(data,"title"), content: formValue(data,"content"), kind: formValue(data,"kind"),
        syntax: formValue(data,"syntax"), access: formValue(data,"access"),
        expiration: expiration ? Math.floor(new Date(expiration).getTime()/1000) : null,
        burn_after_reads: Number(formValue(data,"burn_after_reads") || 0)
      };
      const slug = formValue(data,"slug");
      const paste = slug ? await api<Paste>(`/pastes/${slug}`, { method: "PATCH", body: JSON.stringify(body) }) : await api<Paste>("/pastes", { method: "POST", body: JSON.stringify(body) });
      const files = data.getAll("files").filter(value => value instanceof File && value.size > 0);
      if (files.length) {
        const upload = new FormData();
        files.forEach(file => upload.append("files", file));
        try {
          await api(`/pastes/${paste.slug}/files`, { method: "POST", body: upload });
        } catch (error) {
          if (!slug) await api(`/pastes/${paste.slug}`, { method: "DELETE" }).catch(() => undefined);
          throw error;
        }
      }
      navigate(`/pastes/${paste.slug}`);
    }
    if (form.id === "password-form") {
      await api("/account/password", { method: "PATCH", body: JSON.stringify({ current_password: formValue(data,"current_password"), new_password: formValue(data,"new_password") }) });
      session = state.session = { authenticated:false }; navigate("/login");
    }
    if (form.id === "key-form") {
      const result = await api<{token:string}>("/account/api-keys", { method: "POST", body: JSON.stringify({ name: formValue(data,"name"), scopes: data.getAll("scopes") }) });
      prompt("API key created. Store it now; it will not be shown again.", result.token); await accountView();
    }
    if (form.id === "paste-filters") {
      const params = new URLSearchParams();
      data.forEach((value,key) => { if (value) params.set(key,String(value)); });
      navigate(`${location.pathname}?${params}`);
    }
  } catch (error) {
    notice(error instanceof Error ? error.message : "Request failed", "error");
  } finally {
    controls.forEach(control => control.disabled = false);
  }
});

async function loadAdmin(section: string): Promise<void> {
  const detail = document.querySelector<HTMLElement>("#admin-detail")!;
  detail.classList.remove("hidden");
  if (section === "users") {
    const users = await api<Array<{id:number;username:string;role:string;enabled:boolean}>>("/admin/users");
    detail.innerHTML = `<h2>Users</h2><div class="table">${users.map(u=>`<div><strong>${esc(u.username)}</strong><select data-user-role="${u.id}"><option value="user" ${u.role==="user"?"selected":""}>User</option><option value="admin" ${u.role==="admin"?"selected":""}>Admin</option></select><label class="check"><input type="checkbox" data-user-enabled="${u.id}" ${u.enabled?"checked":""}><span>Enabled</span></label></div>`).join("")}</div>`;
  } else if (section === "invites") {
    const invites = await api<Array<{id:number;token_prefix:string;expires:number;status:string}>>("/admin/invites");
    detail.innerHTML = `<div class="section-heading"><h2>Invitations</h2><button class="button primary" data-action="create-invite">Create invitation</button></div><div class="table">${invites.map(i=>`<div data-id="${i.id}"><code>${esc(i.token_prefix)}…</code><span>${esc(i.status)} · ${date(i.expires)}</span>${i.status==="Active" ? `<button class="button" data-action="revoke-invite">Revoke</button>` : `<span></span>`}</div>`).join("")}</div>`;
  } else {
    const keys = await api<ApiKey[]>("/admin/api-keys");
    detail.innerHTML = `<h2>API keys</h2><div class="table">${keys.map(k=>`<div data-id="${k.id}"><div><strong>${esc(k.name)}</strong><br><code>${esc(k.prefix)}</code></div><label class="check"><input type="checkbox" data-admin-key="${k.id}" ${k.enabled?"checked":""}><span>Enabled</span></label><button class="icon-button" title="Delete API key" aria-label="Delete API key" data-action="delete-admin-key"><i data-icon="trash-2"></i></button></div>`).join("")}</div>`;
  }
  renderIcons();
}

window.addEventListener("popstate", () => void route());
void loadSession().then(route);
