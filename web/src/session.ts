import { getCapabilities, getLanguages, getSession } from "./api";
import { appState } from "./state";
import type { Session } from "./types";

export async function loadSession(): Promise<void> {
  const [session, config, languages] = await Promise.all([
    getSession(),
    getCapabilities(),
    getLanguages()
  ]);
  appState.set({ session, config, languages, ready: true });
}

export function replaceSession(session: Session): void {
  appState.update(state => ({ ...state, session }));
}
