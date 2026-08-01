CREATE TABLE auth_attempts (
    id BIGSERIAL PRIMARY KEY,
    kind TEXT NOT NULL,
    subject TEXT NOT NULL,
    occurred_at BIGINT NOT NULL
);

CREATE INDEX auth_attempts_lookup ON auth_attempts(kind, subject, occurred_at);
