import { requestApi } from "./api";
import { appState } from "./state";
import type { Config, Language, Session } from "./types";

export async function loadSession(): Promise<void> {
  const [session, config, languages] = await Promise.all([
    requestApi<Session>("/session"),
    requestApi<Config>("/capabilities"),
    requestApi<Language[]>("/languages")
  ]);
  appState.set({ session, config, languages, ready: true });
}

export function replaceSession(session: Session): void {
  appState.update(state => ({ ...state, session }));
}
