export type User = {
  id: number;
  username: string;
  role: "user" | "admin";
  force_password_change: boolean;
};

export type Session = { authenticated: boolean; user?: User; csrf_token?: string };
export type PasteFile = { id: number; role: string; name: string; size: number };

export type Paste = {
  id: number;
  slug: string;
  owner_user_id: number | null;
  title: string;
  content: string;
  kind: "text" | "url";
  syntax: string;
  access: "public" | "unlisted" | "owner";
  created: number;
  expiration: number | null;
  read_count: number;
  burn_after_reads: number;
  files: PasteFile[];
};

export type Page<T> = {
  items: T[];
  page: number;
  page_size: number;
  total: number;
};

export type ApiKey = {
  id: number;
  name: string;
  prefix: string;
  scopes: string;
  enabled: boolean;
  created: number;
  last_used: number | null;
};

export type Config = {
  name: string;
  max_file_size: number;
  file_uploads: boolean;
  qr: boolean;
};
