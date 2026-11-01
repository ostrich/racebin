import {
  getCapabilities,
  getLanguages,
  getSession,
  login,
  logout,
  isSessionInvalidError
} from "../api";
import { readonly, writable } from "svelte/store";
import { appState, currentState } from "./state";
import { clearQueryCache } from "./queryCache";
import type { Config, Language, Session } from "../types";

type SessionPhase = "anonymous" | "authenticated";
type SessionOperation = "authenticating" | "refreshing" | "signing-out";
export type AuthenticationState =
  | { phase: "bootstrapping" | "failed" | SessionPhase }
  | { phase: SessionOperation; previous: SessionPhase };

const authentication = writable<AuthenticationState>({ phase: "bootstrapping" });
export const authenticationState = readonly(authentication);
let generation = 0;

function sessionPhase(): SessionPhase {
  return currentState().session.authenticated ? "authenticated" : "anonymous";
}

async function transitionSession(
  phase: SessionOperation,
  request: () => Promise<Session>
): Promise<Session | null> {
  const owner = ++generation;
  authentication.set({ phase, previous: sessionPhase() });
  try {
    const session = await request();
    if (owner !== generation) return null;
    replaceSession(session);
    return session;
  } catch (error) {
    if (owner !== generation) return null;
    authentication.set({ phase: sessionPhase() });
    throw error;
  }
}

export function signIn(credentials: Parameters<typeof login>[0]): Promise<Session | null> {
  return transitionSession("authenticating", async () => ({
    ...(await login(credentials)),
    authenticated: true
  }));
}

export function signOut(): Promise<Session | null> {
  return transitionSession("signing-out", async () => {
    try {
      await logout();
    } catch (error) {
      if (!isSessionInvalidError(error)) throw error;
    }
    return { authenticated: false, permissions: [] };
  });
}

export async function bootstrapApplication(): Promise<void> {
  const owner = ++generation;
  authentication.set({ phase: "bootstrapping" });
  try {
    const [session, config, languages] = await Promise.all([
      getSession(),
      getCapabilities(),
      getLanguages()
    ]);
    if (owner !== generation) return;
    appState.set({ session, config, languages, ready: true });
    authentication.set({ phase: sessionPhase() });
  } catch (error) {
    if (owner !== generation) return;
    authentication.set({ phase: "failed" });
    throw error;
  }
}

export function refreshSession(): Promise<Session | null> {
  return transitionSession("refreshing", getSession);
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
  generation += 1;
  if (currentState().session.user?.id !== session.user?.id) clearQueryCache();
  appState.update((state) => ({ ...state, session }));
  authentication.set({ phase: sessionPhase() });
}
