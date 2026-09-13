import type { components } from "./generated";
import {
  adminUserFromWire,
  adminUserPageFromWire,
  apiKeyFromWire,
  apiKeyPageFromWire,
  auditEventPageFromWire,
  configFromWire,
  folderFromWire,
  folderOverviewFromWire,
  invitationPageFromWire,
  languagesFromWire,
  pasteFromWire,
  pastePageFromWire,
  sessionFromWire,
  type AuditEvent
} from "./normalize";
import { transport, type ApiResult } from "./transport";
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
  PasteRevisionResponse,
  Session,
  User
} from "../types";

type Schema = components["schemas"];
export type CreatePasteInput = Schema["CreatePasteRequest"];
export type UpdatePasteInput = Schema["UpdatePasteRequest"];
export type ConversionInput = Schema["ConversionInput"];
export type Conversion = Schema["ConversionOutput"];
export type LoginInput = Schema["LoginInput"];
export type KeyInput = Schema["KeyInput"];
export type UserUpdate = Schema["UserUpdate"];
export type { Invitation };
export type InvitationCreated = Schema["InvitationCreatedResponse"];
export type FlatCreateInput = Omit<Schema["FlatCreateRequest"], "file">;

async function mapped<W, T>(
  result: Promise<ApiResult<W>>,
  convert: (value: W, etag: string | null) => T
): Promise<T> {
  const response = await result;
  return convert(response.data, response.etag);
}

const id = (value: string | number) => encodeURIComponent(String(value));

export const getSession = () =>
  mapped(transport<Schema["SessionResponse"]>("/session"), sessionFromWire);
export const getCapabilities = () =>
  mapped(transport<Schema["Capabilities"]>("/capabilities"), configFromWire);
export const getLanguages = () =>
  mapped(transport<Schema["Language"][]>("/languages"), languagesFromWire);
export const login = (input: LoginInput) =>
  mapped(
    transport<Schema["SessionCreatedResponse"]>("/session", { method: "POST", json: input }),
    (value) => value
  );
export const logout = () => transport<void>("/session", { method: "DELETE" });
export const changePassword = (input: Schema["PasswordInput"]) =>
  transport<void>("/account/password", { method: "PATCH", json: input });
export const reauthenticate = (password: string) =>
  transport<void>("/session/reauthenticate", { method: "POST", json: { password } });
export const redeemInvitation = (token: string, input: Schema["InvitationInput"]) =>
  transport<void>(`/invitations/${id(token)}/redeem`, { method: "POST", json: input });
export const resetPassword = (token: string, input: Schema["PasswordResetInput"]) =>
  transport<void>(`/password-resets/${id(token)}`, { method: "POST", json: input });

export const listPastes = (query: URLSearchParams) =>
  mapped(transport<Schema["PastePage"]>(`/pastes?${query}`), pastePageFromWire);
export const listAdminPastes = (query: URLSearchParams) =>
  mapped(transport<Schema["PastePage"]>(`/admin/pastes?${query}`), pastePageFromWire);
export const getPaste = (pasteId: string) =>
  mapped(transport<Schema["PasteMetadataResource"]>(`/pastes/${id(pasteId)}`), pasteFromWire);
export const getPasteSource = (pasteId: string) =>
  mapped(transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}/source`), pasteFromWire);
export type ConsumingRead = {
  paste: Paste;
  readToken: string | null;
  idempotencyReplayed: boolean;
};
export async function readPaste(pasteId: string, idempotencyKey: string): Promise<ConsumingRead> {
  const result = await transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}/reads`, {
    method: "POST",
    headers: { "Idempotency-Key": idempotencyKey },
    invalidateQueries: false
  });
  return {
    paste: pasteFromWire(result.data, result.etag),
    readToken: result.readToken,
    idempotencyReplayed: result.idempotencyReplayed
  };
}
export const createPaste = (input: CreatePasteInput, idempotencyKey: string) =>
  mapped(
    transport<Schema["PasteResource"]>("/pastes", {
      method: "POST",
      json: input,
      headers: { "Idempotency-Key": idempotencyKey }
    }),
    pasteFromWire
  );
function multipartBody(input: FlatCreateInput, files: File[]): FormData {
  const body = new FormData();
  for (const [key, value] of Object.entries(input)) {
    if (value !== undefined && value !== null) body.set(key, String(value));
  }
  for (const file of files) body.append("file", file);
  return body;
}

export const createPasteWithAttachments = (
  input: FlatCreateInput,
  files: File[],
  idempotencyKey: string
) =>
  mapped(
    transport<Schema["PasteResource"]>("/pastes", {
      method: "POST",
      body: multipartBody(input, files),
      headers: { "Idempotency-Key": idempotencyKey }
    }),
    pasteFromWire
  );
export const updatePaste = (pasteId: string, input: UpdatePasteInput, etag: string) =>
  mapped(
    transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}`, {
      method: "PATCH",
      json: input,
      headers: { "If-Match": etag }
    }),
    pasteFromWire
  );
export const deletePaste = (pasteId: string, etag: string) =>
  transport<void>(`/pastes/${id(pasteId)}`, { method: "DELETE", headers: { "If-Match": etag } });
export const convertPaste = (input: ConversionInput) =>
  mapped(
    transport<Schema["ConversionOutput"]>("/content-conversions", {
      method: "POST",
      json: input,
      invalidateQueries: false
    }),
    (value) => value
  );
export const uploadAttachments = (pasteId: string, files: File[], etag: string) => {
  const body = new FormData();
  for (const file of files) body.append("file", file);
  return mapped(
    transport(`/pastes/${id(pasteId)}/attachments`, {
      method: "POST",
      body,
      headers: { "If-Match": etag }
    }),
    (value) => value
  );
};
export const deleteAttachment = (pasteId: string, attachmentId: number, etag: string) =>
  transport<void>(`/pastes/${id(pasteId)}/attachments/${id(attachmentId)}`, {
    method: "DELETE",
    headers: { "If-Match": etag }
  });
export const pasteQrUrl = (apiBaseUrl: string, pasteId: string) =>
  `${apiBaseUrl}/pastes/${id(pasteId)}/qr`;

export const listFolders = () =>
  mapped(transport<Schema["FolderOverviewResource"]>("/folders"), folderOverviewFromWire);
export const createFolder = (name: string) =>
  mapped(
    transport<Schema["FolderResource"]>("/folders", {
      method: "POST",
      json: { name } satisfies Schema["FolderInput"]
    }),
    folderFromWire
  );
export const renameFolder = (folderId: number, name: string) =>
  mapped(
    transport<Schema["FolderResource"]>(`/folders/${id(folderId)}`, {
      method: "PATCH",
      json: { name } satisfies Schema["FolderInput"]
    }),
    folderFromWire
  );
export const deleteFolder = (folderId: number) =>
  mapped(
    transport<Schema["PasteRevisionResponse"]>(`/folders/${id(folderId)}`, { method: "DELETE" }),
    (value) => value
  );
export const movePastes = (input: Schema["MovePastesInput"]) =>
  mapped(
    transport<Schema["PasteRevisionResponse"]>("/pastes", { method: "PATCH", json: input }),
    (value) => value
  );

export const listApiKeys = (query = new URLSearchParams()) =>
  mapped(transport<Schema["ApiKeyPage"]>(`/account/api-keys?${query}`), apiKeyPageFromWire);
export const createApiKey = (input: KeyInput) =>
  mapped(
    transport<Schema["ApiKeyCreatedResponse"]>("/account/api-keys", {
      method: "POST",
      json: input
    }),
    (value) => ({ ...value, key: apiKeyFromWire(value.key) })
  );
export const updateApiKey = (keyId: number, enabled: boolean) =>
  transport<void>(`/account/api-keys/${id(keyId)}`, { method: "PATCH", json: { enabled } });
export const deleteApiKey = (keyId: number) =>
  transport<void>(`/account/api-keys/${id(keyId)}`, { method: "DELETE" });

export const listAdminUsers = (query = new URLSearchParams()) =>
  mapped(transport<Schema["AdminUserPage"]>(`/admin/users?${query}`), adminUserPageFromWire);
export const getAdminUser = (userId: number) =>
  mapped(transport<Schema["AdminUserResource"]>(`/admin/users/${id(userId)}`), adminUserFromWire);
export const updateAdminUser = (userId: number, input: UserUpdate) =>
  transport<void>(`/admin/users/${id(userId)}`, { method: "PATCH", json: input });
export const updateAdminUserRole = (userId: number, role: "user" | "admin") =>
  transport<void>(`/admin/users/${id(userId)}/role`, { method: "PATCH", json: { role } });
export const transferOwnership = (userId: number) =>
  transport<void>("/admin/ownership-transfer", { method: "POST", json: { user_id: userId } });
export type InstanceSettings = Schema["InstanceSettingsResource"];
export type { AuditEvent };
export type AdminSummary = Schema["AdminSummaryResource"];
export const getInstanceSettings = () =>
  mapped(transport<InstanceSettings>("/admin/settings"), (value) => value);
export const replaceInstanceSettings = (settings: InstanceSettings) =>
  mapped(
    transport<InstanceSettings>("/admin/settings", { method: "PUT", json: settings }),
    (value) => value
  );
export const getAdminSummary = () =>
  mapped(transport<AdminSummary>("/admin/summary"), (value) => value);
export const listAuditEvents = (query = new URLSearchParams()) =>
  mapped(
    transport<Schema["AuditEventPage"]>(`/admin/audit-events?${query}`),
    auditEventPageFromWire
  );
export const createPasswordReset = (userId: number) =>
  mapped(
    transport<Schema["LinkResponse"]>(`/admin/users/${id(userId)}/password-reset`, {
      method: "POST"
    }),
    (value) => value
  );
export const revokeUserSessions = (userId: number) =>
  transport<void>(`/admin/users/${id(userId)}/sessions`, { method: "DELETE" });
export const revokeUserApiKeys = (userId: number) =>
  transport<void>(`/admin/users/${id(userId)}/api-keys`, { method: "DELETE" });
export const listInvitations = (query = new URLSearchParams()) =>
  mapped(
    transport<Schema["InvitationPage"]>(`/admin/invitations?${query}`),
    invitationPageFromWire
  );
export const createInvitation = (comment?: string) =>
  mapped(
    transport<Schema["InvitationCreatedResponse"]>("/admin/invitations", {
      method: "POST",
      json: { comment }
    }),
    (value) => value
  );
export const updateInvitationComment = (invitationId: number, comment?: string) =>
  transport<void>(`/admin/invitations/${id(invitationId)}`, { method: "PATCH", json: { comment } });
export const revokeInvitation = (invitationId: number) =>
  transport<void>(`/admin/invitations/${id(invitationId)}`, { method: "DELETE" });
export const listAdminApiKeys = (query = new URLSearchParams()) =>
  mapped(transport<Schema["ApiKeyPage"]>(`/admin/api-keys?${query}`), apiKeyPageFromWire);
export const updateAdminApiKey = (keyId: number, enabled: boolean) =>
  transport<void>(`/admin/api-keys/${id(keyId)}`, { method: "PATCH", json: { enabled } });
export const deleteAdminApiKey = (keyId: number) =>
  transport<void>(`/admin/api-keys/${id(keyId)}`, { method: "DELETE" });

export type { ApiResult };
