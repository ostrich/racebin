import { requestApi } from "./api";
import { state } from "./state";
import type { Config, Session } from "./types";

export async function loadSession(): Promise<void> {
  const [session, config] = await Promise.all([
    requestApi<Session>("/session").catch(
      (): Session => ({ authenticated: false })
    ),
    requestApi<Config>("/config")
  ]);
  state.session = session;
  state.config = config;
  if (
    session.user?.password_change_required &&
    location.pathname !== "/account/password"
  ) {
    history.replaceState({}, "", "/account/password");
  }
}
