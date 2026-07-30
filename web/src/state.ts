import type { Config, Session } from "./types";

export const state: { session: Session; config: Config } = {
  session: { authenticated: false },
  config: { name: "Racebin", max_file_size: 0, file_uploads: true, qr: false }
};
