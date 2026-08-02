import { get, writable } from "svelte/store";
import type { Config, Language, Session } from "./types";

export type AppState = {
  session: Session;
  config: Config;
  languages: Language[];
  ready: boolean;
};

export const appState = writable<AppState>({
  session: { authenticated: false },
  config: {
    site_name: "Racebin",
    server_version: "",
    api_version: "v1",
    plain_home_enabled: false,
    max_attachment_size_bytes: 0,
    max_attachments_per_paste: 0,
    attachments_enabled: true,
    qr_codes_enabled: false,
    formats: ["text", "rich_text"],
    visibility_modes: ["public", "unlisted", "private"],
    authentication_methods: ["browser_session", "bearer_api_key"],
    paste_create_media_types: ["application/json"],
    attachment_upload_media_types: ["multipart/form-data"],
    scopes: [],
    max_title_characters: 200,
    max_content_size_bytes: 2 * 1024 * 1024,
    max_page_size: 100,
    minimum_password_characters: 12
  },
  languages: [],
  ready: false
});

export function currentState(): AppState {
  return get(appState);
}
