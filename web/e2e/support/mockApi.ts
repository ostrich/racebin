import type { Page, Route } from "@playwright/test";

const config = {
  site_name: "Racebin",
  plain_home_enabled: false,
  max_attachment_size_bytes: 20 * 1024 * 1024,
  attachments_enabled: true,
  qr_codes_enabled: false
};
const user = {
  id: 1,
  username: "test-admin",
  role: "admin",
  enabled: true,
  password_change_required: false
};
export const paste = {
  id: "sample-paste",
  owner_id: 1,
  folder_id: null,
  title: "JavaScript example",
  content: "const answer = 42;\nconsole.log(answer);",
  document: null,
  content_kind: "text",
  language: "javascript",
  visibility: "unlisted",
  created_at: 1_700_000_000,
  expires_at: null,
  last_read_at: null,
  read_count: 2,
  read_limit: null,
  attachment_count: 1,
  size_bytes: 1064,
  attachments: [{ id: 7, filename: "example.txt", size_bytes: 1024 }]
};
const folderOverview = {
  items: [
    { id: 5, name: "Scripts", created_at: 1_700_000_000, paste_count: 1 },
    { id: 7, name: "sample-folder", created_at: 1_700_000_000, paste_count: 18 }
  ],
  total_count: 19,
  unfiled_count: 0
};

async function json(route: Route, value: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(value)
  });
}

export async function mockApi(
  page: Page,
  authenticated = false,
  options: {
    items?: Array<typeof paste>;
    delay?: number;
    viewPaste?: typeof paste;
    plainHome?: boolean;
    pastePage?: (url: URL) => { items: Array<typeof paste>; delay?: number };
    adminPastePage?: (url: URL) => { items: Array<typeof paste>; delay?: number };
  } = {}
): Promise<void> {
  const viewPaste = options.viewPaste ?? paste;
  let signedIn = authenticated;
  let folders = structuredClone(folderOverview);
  await page.route("**/api/v1/**", async route => {
    const url = new URL(route.request().url());
    if (url.pathname === "/api/v1/session") {
      if (route.request().method() === "POST") signedIn = true;
      return json(route, signedIn
        ? { authenticated: true, user, csrf_token: "csrf" }
        : { authenticated: false });
    }
    if (url.pathname === "/api/v1/config") {
      return json(route, { ...config, plain_home_enabled: options.plainHome ?? false });
    }
    if (url.pathname === "/api/v1/folders") {
      if (route.request().method() === "POST") {
        const body = route.request().postDataJSON() as { name: string };
        const folder = { id: 6, name: body.name, created_at: 1_700_000_001, paste_count: 0 };
        folders.items.push(folder);
        return json(route, folder, 201);
      }
      return json(route, folders);
    }
    if (url.pathname === "/api/v1/folders/5") {
      if (route.request().method() === "PATCH") {
        const body = route.request().postDataJSON() as { name: string };
        folders.items = folders.items.map(folder =>
          folder.id === 5 ? { ...folder, name: body.name } : folder);
        return json(route, folders.items[0]);
      }
      if (route.request().method() === "DELETE") {
        folders.items = folders.items.filter(folder => folder.id !== 5);
        return route.fulfill({ status: 204 });
      }
    }
    if (url.pathname === "/api/v1/pastes/folder") return route.fulfill({ status: 204 });
    if (url.pathname.endsWith("/consume")) return json(route, viewPaste);
    if (url.pathname === "/api/v1/pastes/sample-paste") return json(route, viewPaste);
    if (url.pathname === "/api/v1/pastes/convert") {
      const body = route.request().postDataJSON() as { source_kind: string; content?: string };
      return json(route, body.source_kind === "text"
        ? {
            content: body.content ?? "",
            document: { type: "doc", content: [{ type: "paragraph", content: [] }] }
          }
        : { content: paste.content, document: null });
    }
    if (url.pathname === "/api/v1/account/api-keys") {
      return json(route, [{
        id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
        scopes: ["paste:read", "paste:write"], enabled: true,
        created_at: 1_700_000_000, last_used_at: null
      }]);
    }
    if (url.pathname === "/api/v1/admin/users") return json(route, [user]);
    if (url.pathname === "/api/v1/admin/pastes") {
      const response = options.adminPastePage?.(url) ?? {
        items: options.items ?? [paste],
        delay: options.delay
      };
      if (response.delay) await new Promise(resolve => setTimeout(resolve, response.delay));
      return json(route, {
        items: response.items,
        page: Number(url.searchParams.get("page") ?? 1),
        page_size: 100,
        total_items: response.items.length
      });
    }
    if (url.pathname === "/api/v1/admin/invitations") return json(route, [{
      id: 3, token_prefix: "invite", expires_at: 1_800_000_000,
      status: "Redeemed", redeemed_by_username: "reader"
    }]);
    if (url.pathname === "/api/v1/admin/api-keys") return json(route, [{
      id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
      scopes: ["paste:read", "paste:write"], enabled: true,
      created_at: 1_700_000_000, last_used_at: null
    }]);
    if (url.pathname === "/api/v1/pastes") {
      if (route.request().method() === "POST") return json(route, paste, 201);
      const response = options.pastePage?.(url) ?? {
        items: options.items ?? [paste],
        delay: options.delay
      };
      if (response.delay) await new Promise(resolve => setTimeout(resolve, response.delay));
      return json(route, {
        items: response.items,
        page: Number(url.searchParams.get("page") ?? 1),
        page_size: 50,
        total_items: response.items.length
      });
    }
    return json(route, {});
  });
}
