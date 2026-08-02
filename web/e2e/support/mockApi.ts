import type { Page, Route } from "@playwright/test";

const createdAt = "2023-11-14T22:13:20Z";
const joinedAt = "2023-07-22T04:26:40Z";
const expiresAt = "2027-01-15T08:00:00Z";

const config = {
  site_name: "Racebin",
  server_version: "0.1.0",
  api_version: "v1",
  web_base_url: "http://127.0.0.1:4173",
  api_base_url: "http://127.0.0.1:4173/api/v1",
  plain_home_enabled: false,
  max_attachment_size_bytes: 20 * 1024 * 1024,
  max_attachments_per_paste: 32,
  attachments_enabled: true,
  qr_codes_enabled: false,
  formats: ["text", "rich_text"],
  visibility_modes: ["public", "unlisted", "private"],
  authentication_methods: ["browser_session", "bearer_api_key"],
  paste_create_media_types: ["application/json", "multipart/form-data"],
  attachment_upload_media_types: ["multipart/form-data"],
  scopes: [
    { id: "paste:read", description: "Read paste content available to the key owner" },
    { id: "paste:write", description: "Create and update pastes, folders, and attachments" },
    { id: "paste:delete", description: "Delete owned pastes" },
    { id: "paste:list", description: "List and search non-public pastes and folders" }
  ],
  max_title_characters: 200,
  max_content_size_bytes: 2 * 1024 * 1024,
  max_page_size: 100,
  minimum_password_characters: 12
};
const languages = [
  { id: "plaintext", label: "Plain text", aliases: ["text", "txt"] },
  { id: "javascript", label: "JavaScript", aliases: ["js", "jsx"] }
];
const user = {
  id: 1,
  username: "test-admin",
  role: "admin",
  enabled: true,
  password_change_required: false,
  created_at: joinedAt,
  last_login_at: createdAt,
  paste_count: 3,
  storage_bytes: 4096,
  active_session_count: 1,
  api_key_count: 2,
  active_api_key_count: 1
};
export const paste = {
  id: "sample-paste",
  url: "/pastes/sample-paste",
  api_url: "/api/v1/pastes/sample-paste",
  read_url: "/api/v1/pastes/sample-paste/reads",
  source_url: "/api/v1/pastes/sample-paste/source",
  owner_id: 1,
  folder_id: null,
  title: "JavaScript example",
  content: "const answer = 42;\nconsole.log(answer);",
  document: null,
  content_kind: "text",
  format: "text" as const,
  body: {
    format: "text" as const,
    content: "const answer = 42;\nconsole.log(answer);",
    language: "javascript"
  },
  language: "javascript",
  visibility: "unlisted",
  created_at: createdAt,
  updated_at: createdAt,
  expires_at: null,
  last_read_at: null,
  read_count: 2,
  read_limit: null,
  attachment_count: 1,
  size_bytes: 1064,
  attachments: [{
    id: 7,
    filename: "example.txt",
    size_bytes: 1024,
    url: "/api/v1/pastes/sample-paste/attachments/7"
  }]
};
const folderOverview = {
  items: [
    { id: 5, name: "Scripts", created_at: createdAt, paste_count: 1 },
    { id: 7, name: "sample-folder", created_at: createdAt, paste_count: 18 }
  ],
  total_count: 19,
  unfiled_count: 0
};

function wireMockValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(wireMockValue);
  if (!value || typeof value !== "object") return value;
  const object = value as Record<string, unknown>;
  if (typeof object.id === "string" && typeof object.content_kind === "string") {
    const id = object.id;
    const richText = object.content_kind === "rich_text";
    return {
      ...object,
      url: `/pastes/${id}`,
      api_url: `/api/v1/pastes/${id}`,
      read_url: `/api/v1/pastes/${id}/reads`,
      source_url: `/api/v1/pastes/${id}/source`,
      format: object.content_kind,
      body: richText
        ? { format: "rich_text", content: object.document ?? "", plain_text: object.content ?? "" }
        : { format: "text", content: object.content ?? "", language: object.language ?? "plaintext" },
      created_at: typeof object.created_at === "number"
        ? new Date(object.created_at * 1000).toISOString()
        : object.created_at,
      updated_at: typeof object.updated_at === "number"
        ? new Date(object.updated_at * 1000).toISOString()
        : object.updated_at ?? object.created_at,
      attachments: Array.isArray(object.attachments)
        ? object.attachments.map(item => {
            const attachment = item as Record<string, unknown>;
            return {
              ...attachment,
              url: attachment.url ?? `/api/v1/pastes/${id}/attachments/${attachment.id}`
            };
          })
        : []
    };
  }
  return Object.fromEntries(Object.entries(object).map(([key, item]) => [key, wireMockValue(item)]));
}

async function json(route: Route, value: unknown, status = 200): Promise<void> {
  await route.fulfill({
    status,
    contentType: "application/json",
    body: JSON.stringify(wireMockValue(value))
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
    if (url.pathname === "/api/v1/capabilities") {
      return json(route, { ...config, plain_home_enabled: options.plainHome ?? false });
    }
    if (url.pathname === "/api/v1/languages") return json(route, languages);
    if (url.pathname === "/api/v1/folders") {
      if (route.request().method() === "POST") {
        const body = route.request().postDataJSON() as { name: string };
        const folder = { id: 6, name: body.name, created_at: "2023-11-14T22:13:21Z", paste_count: 0 };
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
    if (url.pathname === "/api/v1/pastes" && route.request().method() === "PATCH") return route.fulfill({ status: 204 });
    if (url.pathname.endsWith("/reads")) return json(route, viewPaste);
    if (url.pathname === "/api/v1/pastes/sample-paste/source") return json(route, viewPaste);
    if (url.pathname === "/api/v1/pastes/sample-paste") return json(route, viewPaste);
    if (url.pathname === "/api/v1/content-conversions") {
      const body = route.request().postDataJSON() as { source: { format: string; content: string }; target_format: string };
      return json(route, body.target_format === "rich_text"
        ? { body: { format: "rich_text", content: `<p>${body.source.content}</p>` } }
        : { body: { format: "text", content: paste.content, language: "plaintext" } });
    }
    if (url.pathname === "/api/v1/account/api-keys") {
      return json(route, [{
        id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
        scopes: ["paste:read", "paste:write"], enabled: true,
        created_at: createdAt, last_used_at: null
      }]);
    }
    if (url.pathname === "/api/v1/admin/users") return json(route, [user]);
    if (url.pathname === "/api/v1/admin/users/1") {
      if (route.request().method() === "PATCH") return json(route, {});
      return json(route, user);
    }
    if (url.pathname === "/api/v1/admin/users/1/password-reset") {
      return json(route, { url: "/password-reset/sample-reset-token" }, 201);
    }
    if (["/api/v1/admin/users/1/sessions", "/api/v1/admin/users/1/api-keys"].includes(url.pathname)) {
      return route.fulfill({ status: 204 });
    }
    if (url.pathname === "/api/v1/password-resets/sample-reset-token") return route.fulfill({ status: 204 });
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
    if (url.pathname === "/api/v1/admin/invitations") {
      if (route.request().method() === "POST") {
        return json(route, { token: "new-token", url: "/invitations/new-token" }, 201);
      }
      return json(route, [
        {
          id: 4, token_prefix: "active", expires_at: expiresAt,
          status: "Active", url: "/invitations/active-token", redeemed_by_username: null
        },
        {
          id: 3, token_prefix: "invite", expires_at: expiresAt,
          status: "Redeemed", url: null, redeemed_by_username: "reader"
        }
      ]);
    }
    if (url.pathname === "/api/v1/admin/api-keys") return json(route, [{
      id: 4, user_id: 1, name: "Automation", token_prefix: "abcd",
      scopes: ["paste:read", "paste:write"], enabled: true,
      created_at: createdAt, last_used_at: null
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
