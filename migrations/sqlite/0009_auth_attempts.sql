CREATE TABLE auth_attempts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    subject TEXT NOT NULL,
    occurred_at INTEGER NOT NULL
);

CREATE INDEX auth_attempts_lookup ON auth_attempts(kind, subject, occurred_at);
