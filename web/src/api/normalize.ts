import type { components } from "./generated";
import type {
  AdminUser,
  ApiKey,
  Config,
  Folder,
  FolderOverview,
  Invitation,
  Language,
  Page,
  Paste,
  Session
} from "../types";

type Schema = components["schemas"];
type WirePaste = Schema["PasteResource"] | Schema["PasteMetadataResource"] | Schema["PasteSummary"];

function isPasteResource(value: WirePaste): value is Schema["PasteResource"] {
  return "body" in value;
}

export function unixTimestamp(value: string | null | undefined): number | null {
  if (value === null || value === undefined) return null;
  const milliseconds = Date.parse(value);
  return Number.isFinite(milliseconds) ? Math.floor(milliseconds / 1000) : null;
}

export function pasteFromWire(value: WirePaste, etag?: string | null): Paste {
  const resource = "attachments" in value ? value : undefined;
  const body = isPasteResource(value) ? value.body : undefined;
  return {
    id: value.id,
    url: value.url,
    api_url: resource?.api_url,
    read_url: resource?.read_url,
    raw_url: resource?.raw_url ?? undefined,
    source_url: resource?.source_url ?? undefined,
    archive_url: resource?.archive_url ?? undefined,
    _etag: etag ?? undefined,
    owner_id: value.owner_id ?? null,
    owner_username: "owner_username" in value ? (value.owner_username ?? undefined) : undefined,
    folder_id: value.folder_id ?? null,
    title: value.title,
    content: body?.content ?? ("excerpt" in value ? (value.excerpt ?? "") : ""),
    rendered_html: body?.format === "markdown" ? body.rendered_html : null,
    plain_text: body?.format === "markdown" ? body.plain_text : (body?.content ?? ""),
    format: value.format === "markdown" ? "markdown" : "text",
    language: body?.format === "text" ? body.language : (value.language ?? "plaintext"),
    visibility:
      value.visibility === "public" || value.visibility === "private"
        ? value.visibility
        : "unlisted",
    created_at: unixTimestamp(value.created_at) ?? 0,
    updated_at: unixTimestamp(value.updated_at) ?? unixTimestamp(value.created_at) ?? 0,
    modified_at: unixTimestamp(value.modified_at),
    expires_at: unixTimestamp(value.expires_at),
    last_read_at: unixTimestamp(value.last_read_at),
    read_count: value.read_count,
    read_limit: value.read_limit ?? null,
    attachment_count: value.attachment_count,
    attachment_only_filename: value.attachment_only_filename ?? undefined,
    size_bytes: value.size_bytes,
    attachments:
      resource?.attachments.map((attachment) => ({
        id: attachment.id,
        filename: attachment.filename,
        size_bytes: attachment.size_bytes,
        url: attachment.url
      })) ?? []
  };
}

function pageFromWire<W, T>(
  value: { items: W[]; pagination: Schema["Pagination"] },
  convert: (item: W) => T
): Page<T> {
  return { ...value.pagination, items: value.items.map(convert) };
}

export const pastePageFromWire = (value: Schema["PastePage"]): Page<Paste> =>
  pageFromWire(value, (paste) => pasteFromWire(paste));

export function sessionFromWire(value: Schema["SessionResponse"]): Session {
  return value;
}

export function configFromWire(value: Schema["Capabilities"]): Config {
  return {
    ...value,
    web_base_url: value.web_base_url ?? undefined,
    api_base_url: value.api_base_url ?? undefined,
    default_expiration_seconds: value.default_expiration_seconds ?? null,
    plain_home_enabled: value.plain_home_enabled,
    default_format: value.default_format === "markdown" ? "markdown" : "text",
    default_visibility:
      value.default_visibility === "public" || value.default_visibility === "private"
        ? value.default_visibility
        : "unlisted",
    formats: value.formats.filter(
      (format): format is "text" | "markdown" => format === "text" || format === "markdown"
    ),
    visibility_modes: value.visibility_modes.filter(
      (visibility): visibility is "public" | "unlisted" | "private" =>
        visibility === "public" || visibility === "unlisted" || visibility === "private"
    )
  };
}

export function languagesFromWire(value: Schema["Language"][]): Language[] {
  return value.map((language) => ({ ...language }));
}

export function folderFromWire(value: Schema["FolderResource"]): Folder {
  return { ...value, created_at: unixTimestamp(value.created_at) ?? 0 };
}

export function folderOverviewFromWire(value: Schema["FolderOverviewResource"]): FolderOverview {
  return { ...value, items: value.items.map(folderFromWire) };
}

export function apiKeyFromWire(value: Schema["ApiKeyResource"]): ApiKey {
  return {
    ...value,
    user_id: value.user_id ?? null,
    owner_username: value.owner_username ?? null,
    created_at: unixTimestamp(value.created_at) ?? 0,
    last_used_at: unixTimestamp(value.last_used_at)
  };
}

export const apiKeyPageFromWire = (value: Schema["ApiKeyPage"]): Page<ApiKey> =>
  pageFromWire(value, apiKeyFromWire);

export function adminUserFromWire(value: Schema["AdminUserResource"]): AdminUser {
  return {
    ...value,
    created_at: unixTimestamp(value.created_at) ?? 0,
    last_login_at: unixTimestamp(value.last_login_at)
  };
}

export const adminUserPageFromWire = (value: Schema["AdminUserPage"]): Page<AdminUser> =>
  pageFromWire(value, adminUserFromWire);

export function invitationFromWire(value: Schema["InvitationResource"]): Invitation {
  return {
    ...value,
    created_at: unixTimestamp(value.created_at) ?? 0,
    expires_at: unixTimestamp(value.expires_at) ?? 0,
    redeemed_at: unixTimestamp(value.redeemed_at) ?? undefined
  };
}

export const invitationPageFromWire = (value: Schema["InvitationPage"]): Page<Invitation> =>
  pageFromWire(value, invitationFromWire);

export type AuditEvent = Omit<Schema["AuditEventResource"], "created_at"> & { created_at: number };
export const auditEventPageFromWire = (value: Schema["AuditEventPage"]): Page<AuditEvent> =>
  pageFromWire(value, (event) => ({
    ...event,
    created_at: unixTimestamp(event.created_at) ?? 0
  }));
