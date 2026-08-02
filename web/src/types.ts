export type User = {
  id: number;
  username: string;
  role: "user" | "admin";
  enabled?: boolean;
  password_change_required: boolean;
};

export type AdminUser = User & {
  enabled: boolean;
  created_at: number;
  last_login_at: number | null;
  paste_count: number;
  storage_bytes: number;
  active_session_count: number;
  api_key_count: number;
  active_api_key_count: number;
};

export type Session =
  | { authenticated: false; user?: never; api_key?: never; csrf_token?: never }
  | { authenticated: true; user: User; csrf_token: string }
  | { authenticated: true; api_key: { id: number; name: string; scopes: string[] }; user?: never; csrf_token?: never };
export type Attachment = { id: number; filename: string; size_bytes: number; url: string };
export type RichTextDocument = Record<string, unknown> | string;

export type Paste = {
  id: string;
  url?: string;
  api_url?: string;
  read_url?: string;
  source_url?: string;
  archive_url?: string;
  _etag?: string;
  owner_id: number | null;
  folder_id: number | null;
  title: string;
  content: string;
  document: RichTextDocument | null;
  content_kind: "text" | "rich_text";
  language: string;
  visibility: "public" | "unlisted" | "private";
  created_at: number;
  updated_at?: number;
  expires_at: number | null;
  last_read_at: number | null;
  read_count: number;
  read_limit: number | null;
  attachment_count: number;
  size_bytes: number;
  attachments: Attachment[];
};

export type Folder = {
  id: number;
  name: string;
  created_at: number;
  paste_count: number;
};

export type FolderOverview = {
  items: Folder[];
  total_count: number;
  unfiled_count: number;
};

export type PasteRevisionResponse = {
  pastes: Array<{ id: string; etag: string }>;
};

export type Page<T> = {
  items: T[];
  page: number;
  page_size: number;
  total_items: number;
};

export type ApiKey = {
  id: number;
  user_id: number | null;
  name: string;
  token_prefix: string;
  scopes: string[];
  enabled: boolean;
  created_at: number;
  last_used_at: number | null;
};

export type Config = {
  site_name: string;
  server_version: string;
  api_version: string;
  web_base_url?: string;
  api_base_url?: string;
  plain_home_enabled: boolean;
  max_attachment_size_bytes: number;
  max_attachments_per_paste: number;
  attachments_enabled: boolean;
  qr_codes_enabled: boolean;
  formats: Array<"text" | "rich_text">;
  visibility_modes: Array<"public" | "unlisted" | "private">;
  authentication_methods: string[];
  paste_create_media_types: string[];
  attachment_upload_media_types: string[];
  scopes: Array<{ id: string; description: string }>;
  max_title_characters: number;
  max_content_size_bytes: number;
  max_page_size: number;
  minimum_password_characters: number;
};

export type Language = { id: string; label: string; aliases: string[] };
