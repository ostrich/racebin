import { get, writable } from "svelte/store";
import type { Config, Session } from "./types";

export type AppState = {
  session: Session;
  config: Config;
  ready: boolean;
};

export const appState = writable<AppState>({
  session: { authenticated: false },
  config: {
    site_name: "Racebin",
    plain_home_enabled: false,
    max_attachment_size_bytes: 0,
    attachments_enabled: true,
    qr_codes_enabled: false
  },
  ready: false
});

export function currentState(): AppState {
  return get(appState);
}
