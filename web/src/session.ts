import { api } from "./api";
import { state } from "./state";
import type { Config, Session } from "./types";

export async function loadSession(): Promise<void> {
  const [session, config] = await Promise.all([
    api<Session>("/session").catch(
      (): Session => ({ authenticated: false })
    ),
    api<Config>("/config")
  ]);
  state.session = session;
  state.config = config;
  if (
    session.user?.force_password_change &&
    location.pathname !== "/account/password"
  ) {
    history.replaceState({}, "", "/account/password");
  }
}
