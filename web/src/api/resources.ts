import type { components } from "./generated";
import { normalizePayload } from "./normalize";
import { transport, type ApiResult } from "./transport";
import type {
  AdminUser, ApiKey, Config, Folder, FolderOverview, Language, Page, Paste,
  PasteRevisionResponse, Session, User
} from "../types";

type Schema = components["schemas"];
export type CreatePasteInput = Schema["CreatePasteRequest"];
export type UpdatePasteInput = Schema["UpdatePasteRequest"];
export type ConversionInput = Schema["ConversionInput"];
export type Conversion = Schema["ConversionOutput"];
export type LoginInput = Schema["LoginInput"];
export type KeyInput = Schema["KeyInput"];
export type UserUpdate = Schema["UserUpdate"];
export type Invitation = Omit<Schema["InvitationResource"], "expires_at"> & { expires_at: number };
export type FlatCreateInput = Omit<Schema["FlatCreateRequest"], "file">;

async function normalized<T>(result: Promise<ApiResult<unknown>>): Promise<T> {
  const response = await result;
  return normalizePayload(response.data, response.etag) as T;
}

const id = (value: string | number) => encodeURIComponent(String(value));

export const getSession = () => normalized<Session>(transport<Schema["SessionResponse"]>("/session"));
export const getCapabilities = () => normalized<Config>(transport<Schema["Capabilities"]>("/capabilities"));
export const getLanguages = () => normalized<Language[]>(transport<Schema["Language"][]>("/languages"));
export const login = (input: LoginInput) => normalized<Schema["SessionCreatedResponse"]>(
  transport("/session", { method: "POST", json: input })
);
export const logout = () => transport<void>("/session", { method: "DELETE" });
export const changePassword = (input: Schema["PasswordInput"]) => transport<void>("/account/password", { method: "PATCH", json: input });
export const redeemInvitation = (token: string, input: Schema["InvitationInput"]) =>
  transport<void>(`/invitations/${id(token)}/redeem`, { method: "POST", json: input });
export const resetPassword = (token: string, input: Schema["PasswordResetInput"]) =>
  transport<void>(`/password-resets/${id(token)}`, { method: "POST", json: input });

export const listPastes = (query: URLSearchParams) =>
  normalized<Page<Paste>>(transport<Schema["PastePage"]>(`/pastes?${query}`));
export const listAdminPastes = (query: URLSearchParams) =>
  normalized<Page<Paste>>(transport<Schema["PastePage"]>(`/admin/pastes?${query}`));
export const getPaste = (pasteId: string) => normalized<Paste>(transport<Schema["PasteMetadataResource"]>(`/pastes/${id(pasteId)}`));
export const getPasteSource = (pasteId: string) => normalized<Paste>(transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}/source`));
export type ConsumingRead = { paste: Paste; readToken: string | null; idempotencyReplayed: boolean };
export async function readPaste(pasteId: string, idempotencyKey: string): Promise<ConsumingRead> {
  const result = await transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}/reads`, {
    method: "POST", headers: { "Idempotency-Key": idempotencyKey }, invalidateQueries: false
  });
  return {
    paste: normalizePayload(result.data, result.etag) as Paste,
    readToken: result.readToken,
    idempotencyReplayed: result.idempotencyReplayed
  };
}
export const createPaste = (input: CreatePasteInput, idempotencyKey: string) => normalized<Paste>(
  transport<Schema["PasteResource"]>("/pastes", {
    method: "POST", json: input, headers: { "Idempotency-Key": idempotencyKey }
  })
);
function multipartBody(input: FlatCreateInput, files: File[]): FormData {
  const body = new FormData();
  for (const [key, value] of Object.entries(input)) {
    if (value !== undefined && value !== null) body.set(key, String(value));
  }
  for (const file of files) body.append("file", file);
  return body;
}

export const createPasteWithAttachments = (input: FlatCreateInput, files: File[], idempotencyKey: string) => normalized<Paste>(
  transport<Schema["PasteResource"]>("/pastes", {
    method: "POST", body: multipartBody(input, files), headers: { "Idempotency-Key": idempotencyKey }
  })
);
export const updatePaste = (pasteId: string, input: UpdatePasteInput, etag: string) => normalized<Paste>(
  transport<Schema["PasteResource"]>(`/pastes/${id(pasteId)}`, {
    method: "PATCH", json: input, headers: { "If-Match": etag }
  })
);
export const deletePaste = (pasteId: string, etag: string) =>
  transport<void>(`/pastes/${id(pasteId)}`, { method: "DELETE", headers: { "If-Match": etag } });
export const convertPaste = (input: ConversionInput) =>
  normalized<Conversion>(transport<Schema["ConversionOutput"]>("/content-conversions", {
    method: "POST", json: input, invalidateQueries: false
  }));
export const uploadAttachments = (pasteId: string, files: File[], etag: string) => {
  const body = new FormData();
  for (const file of files) body.append("file", file);
  return (
  normalized<Schema["AttachmentUploadResponse"]>(transport(`/pastes/${id(pasteId)}/attachments`, {
    method: "POST", body, headers: { "If-Match": etag }
  })));
};
export const deleteAttachment = (pasteId: string, attachmentId: number, etag: string) =>
  transport<void>(`/pastes/${id(pasteId)}/attachments/${id(attachmentId)}`, {
    method: "DELETE", headers: { "If-Match": etag }
  });
export const pasteQrUrl = (apiBaseUrl: string, pasteId: string) => `${apiBaseUrl}/pastes/${id(pasteId)}/qr`;

export const listFolders = () => normalized<FolderOverview>(transport<Schema["FolderOverviewResource"]>("/folders"));
export const createFolder = (name: string) => normalized<Folder>(transport<Schema["FolderResource"]>("/folders", {
  method: "POST", json: { name } satisfies Schema["FolderInput"]
}));
export const renameFolder = (folderId: number, name: string) => normalized<Folder>(
  transport<Schema["FolderResource"]>(`/folders/${id(folderId)}`, {
    method: "PATCH", json: { name } satisfies Schema["FolderInput"]
  })
);
export const deleteFolder = (folderId: number) => normalized<PasteRevisionResponse>(
  transport<Schema["PasteRevisionResponse"]>(`/folders/${id(folderId)}`, { method: "DELETE" })
);
export const movePastes = (input: Schema["MovePastesInput"]) => normalized<PasteRevisionResponse>(
  transport<Schema["PasteRevisionResponse"]>("/pastes", { method: "PATCH", json: input })
);

export const listApiKeys = () => normalized<ApiKey[]>(transport<Schema["ApiKeyResource"][]>("/account/api-keys"));
export const createApiKey = (input: KeyInput) => normalized<{ key: ApiKey; token: string }>(
  transport<Schema["ApiKeyCreatedResponse"]>("/account/api-keys", { method: "POST", json: input })
);
export const updateApiKey = (keyId: number, enabled: boolean) =>
  transport<void>(`/account/api-keys/${id(keyId)}`, { method: "PATCH", json: { enabled } });
export const deleteApiKey = (keyId: number) =>
  transport<void>(`/account/api-keys/${id(keyId)}`, { method: "DELETE" });

export const listAdminUsers = () => normalized<AdminUser[]>(transport<Schema["AdminUserResource"][]>("/admin/users"));
export const getAdminUser = (userId: number) => normalized<AdminUser>(transport<Schema["AdminUserResource"]>(`/admin/users/${id(userId)}`));
export const updateAdminUser = (userId: number, input: UserUpdate) =>
  transport<void>(`/admin/users/${id(userId)}`, { method: "PATCH", json: input });
export const createPasswordReset = (userId: number) => normalized<Schema["LinkResponse"]>(
  transport<Schema["LinkResponse"]>(`/admin/users/${id(userId)}/password-reset`, { method: "POST" })
);
export const revokeUserSessions = (userId: number) =>
  transport<void>(`/admin/users/${id(userId)}/sessions`, { method: "DELETE" });
export const revokeUserApiKeys = (userId: number) =>
  transport<void>(`/admin/users/${id(userId)}/api-keys`, { method: "DELETE" });
export const listInvitations = () => normalized<Invitation[]>(transport<Schema["InvitationResource"][]>("/admin/invitations"));
export const createInvitation = () => normalized<Schema["InvitationCreatedResponse"]>(
  transport<Schema["InvitationCreatedResponse"]>("/admin/invitations", { method: "POST" })
);
export const revokeInvitation = (invitationId: number) =>
  transport<void>(`/admin/invitations/${id(invitationId)}`, { method: "DELETE" });
export const listAdminApiKeys = () => normalized<ApiKey[]>(transport<Schema["ApiKeyResource"][]>("/admin/api-keys"));
export const updateAdminApiKey = (keyId: number, enabled: boolean) =>
  transport<void>(`/admin/api-keys/${id(keyId)}`, { method: "PATCH", json: { enabled } });
export const deleteAdminApiKey = (keyId: number) =>
  transport<void>(`/admin/api-keys/${id(keyId)}`, { method: "DELETE" });

export type { ApiResult };
