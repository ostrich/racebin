# Temporary SQLx SQLite dependency backport

`sqlx-sqlite/` contains the published `sqlx-sqlite` 0.9.0 crate's source,
manifests, and MIT/Apache-2.0 licenses. Its Rust source is unchanged.

The only modification is the normalized `Cargo.toml` dependency range for
`libsqlite3-sys`: `>=0.30.1, <0.38.0` becomes `>=0.39.0, <0.40.0`.
This permits and requires the SQLite security fix for
[RUSTSEC-2026-0459](https://rustsec.org/advisories/RUSTSEC-2026-0459).
The original published manifest remains in `Cargo.toml.orig` for comparison.

Upstream already widened its upper bound in
[SQLx PR #4502](https://github.com/launchbadge/sqlx/pull/4502), commit
`10462fa4414c6c7f1ed1d39bb31678e3b726878c`, but has not published that change.
This backport avoids adopting unrelated unreleased SQLx changes or depending
on the host's SQLite version, including during cross-compilation.

Remove `sqlx-sqlite/` and the root `[patch.crates-io]` entry when a released
SQLx version supports `libsqlite3-sys` 0.39 or newer. Update the lockfile,
regenerate the Rust license notice, and run both SQLite and PostgreSQL
integration suites when doing so. Do not refactor or reformat this upstream
source as part of Racebin changes.

The scheduled/manual dependency-audit job runs
`python3 -B scripts/check-sqlx-backport.py`. It resolves the latest published
stable SQLx independently of this repository's patch and lockfile, without
building it. Once that release selects patched SQLite, the check fails with
removal instructions in the CI annotation and job summary. It does not run in
the normal push gate, and becomes a no-op after the patch is removed.
