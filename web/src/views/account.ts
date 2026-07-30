import { requestApi } from "../api";
import { navigate } from "../router";
import { state } from "../state";
import type { ApiKey } from "../types";
import { escapeHtml, iconButton, renderLayout } from "../ui";

export function loginView(): void {
  renderLayout(`<section class="auth"><form id="login-form"><p class="eyebrow">Account</p><h1>Log in</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required autofocus></label>
    <label><span>Password</span><input type="password" name="password" autocomplete="current-password" required></label>
    <label class="check"><input type="checkbox" name="remember"><span>Keep me signed in</span></label>
    <button class="button primary" type="submit">Log in</button></form></section>`);
}

export function invitationView(token: string): void {
  renderLayout(`<section class="auth"><form id="invitation-form"><p class="eyebrow">Invitation</p><h1>Create your account</h1>
    <label><span>Username</span><input name="username" autocomplete="username" required></label>
    <label><span>Password</span><input type="password" name="password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Create account</button><input type="hidden" name="token" value="${escapeHtml(token)}"></form></section>`);
}

export async function accountView(): Promise<void> {
  if (!state.session.user) return navigate("/login");
  const keys = await requestApi<ApiKey[]>("/account/api-keys");
  renderLayout(`<section><div class="page-heading"><div><p class="eyebrow">Settings</p><h1>Account</h1></div><a class="button" href="/account/password" data-link>Change password</a></div>
    <section class="panel"><h2>API keys</h2><p class="muted">Tokens are shown once when created.</p>
      <div class="key-list">${keys.length ? keys.map(key => `<div class="key-row"><div><strong>${escapeHtml(key.name)}</strong><code>rbk_${escapeHtml(key.token_prefix)}_...</code><small>${escapeHtml(key.scopes.join(", "))}</small></div><label class="switch"><input type="checkbox" data-key="${key.id}" ${key.enabled ? "checked" : ""}><span></span></label>${iconButton("trash-2", "Delete API key")}<input type="hidden" value="${key.id}"></div>`).join("") : `<p class="empty compact">No API keys.</p>`}</div>
      <form id="key-form" class="key-form"><label><span>Name</span><input name="name" required maxlength="100"></label>
      <fieldset><legend>Scopes</legend><div class="scope-options">${["paste:read","paste:write","paste:delete","paste:list"].map(v => `<label class="check"><input type="checkbox" name="scopes" value="${v}"><span>${v}</span></label>`).join("")}</div></fieldset>
      <button class="button primary" type="submit"><i data-icon="key-round"></i> Create key</button></form>
    </section></section>`);
}

export function passwordView(): void {
  if (!state.session.user) return navigate("/login");
  renderLayout(`<section class="auth"><form id="password-form"><p class="eyebrow">Security</p><h1>Change password</h1>
    <label><span>Current password</span><input type="password" name="current_password" autocomplete="current-password" required></label>
    <label><span>New password</span><input type="password" name="new_password" minlength="12" autocomplete="new-password" required></label>
    <button class="button primary" type="submit">Update password</button></form></section>`);
}
