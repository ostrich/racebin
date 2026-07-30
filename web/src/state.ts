import type { Config, Session } from "./types";

export const state: { session: Session; config: Config } = {
  session: { authenticated: false },
  config: {
    site_name: "Racebin",
    max_attachment_size_bytes: 0,
    attachments_enabled: true,
    qr_codes_enabled: false
  }
};
