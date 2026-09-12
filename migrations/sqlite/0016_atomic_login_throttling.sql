CREATE TABLE auth_buckets (
  kind TEXT NOT NULL,
  subject TEXT NOT NULL,
  window_started_at BIGINT NOT NULL,
  attempt_count BIGINT NOT NULL CHECK(attempt_count >= 0),
  PRIMARY KEY(kind, subject)
);

