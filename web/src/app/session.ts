import { getCapabilities, getLanguages, getSession } from "../api";
import { appState } from "./state";
import type { Config, Language, Session } from "../types";

export async function bootstrapApplication(): Promise<void> {
  const [session, config, languages] = await Promise.all([
    getSession(),
    getCapabilities(),
    getLanguages()
  ]);
  appState.set({ session, config, languages, ready: true });
}

export async function refreshSession(): Promise<Session> {
  const session = await getSession();
  replaceSession(session);
  return session;
}

export async function refreshCapabilities(): Promise<Config> {
  const config = await getCapabilities();
  appState.update((state) => ({ ...state, config }));
  return config;
}

export async function refreshLanguages(): Promise<Language[]> {
  const languages = await getLanguages();
  appState.update((state) => ({ ...state, languages }));
  return languages;
}

export function replaceSession(session: Session): void {
  appState.update((state) => ({ ...state, session }));
}
