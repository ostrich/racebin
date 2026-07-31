export type User = {
  id: number;
  username: string;
  role: "user" | "admin";
  enabled?: boolean;
  password_change_required: boolean;
};

export type Session = { authenticated: boolean; user?: User; csrf_token?: string };
export type Attachment = { id: number; filename: string; size_bytes: number };
export type RichTextDocument = Record<string, unknown>;

export type Paste = {
  id: string;
  owner_id: number | null;
  folder_id: number | null;
  title: string;
  content: string;
  document: RichTextDocument | null;
  content_kind: "text" | "rich_text";
  language: string;
  visibility: "public" | "unlisted" | "private";
  created_at: number;
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
  max_attachment_size_bytes: number;
  attachments_enabled: boolean;
  qr_codes_enabled: boolean;
};
