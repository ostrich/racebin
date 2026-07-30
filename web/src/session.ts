import { requestApi } from "./api";
import { appState } from "./state";
import type { Config, Session } from "./types";

export async function loadSession(): Promise<void> {
  const [session, config] = await Promise.all([
    requestApi<Session>("/session").catch(
      (): Session => ({ authenticated: false })
    ),
    requestApi<Config>("/config")
  ]);
  appState.set({ session, config, ready: true });
}

export function replaceSession(session: Session): void {
  appState.update(state => ({ ...state, session }));
}
