#!/usr/bin/env python3
"""Remind maintainers when published SQLx can replace the SQLite backport."""

import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


def replacement(lock):
    packages = {package["name"]: package for package in lock["package"]}
    sqlx = packages["sqlx"]["version"]
    sqlite = packages["libsqlite3-sys"]["version"]
    # Cargo chooses stable releases for the probe. Compare numerically, not
    # lexically (0.100 must compare greater than 0.39).
    version = tuple(int(part) for part in sqlite.split("."))
    return (sqlx, sqlite) if version >= (0, 39, 0) else None


def main():
    root = Path(__file__).resolve().parent.parent
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    if "sqlx-sqlite" not in manifest.get("patch", {}).get("crates-io", {}):
        print("SQLx SQLite backport is absent; no reminder needed.")
        return 0

    # Resolve outside the repository so its patch and lockfile cannot influence
    # the result. This only resolves dependencies; it never builds upstream code.
    with tempfile.TemporaryDirectory(prefix="racebin-sqlx-release-") as directory:
        probe = Path(directory)
        (probe / "Cargo.toml").write_text(
            '[package]\nname = "sqlx-release-check"\nversion = "0.0.0"\n'
            '[lib]\npath = "lib.rs"\n'
            '[dependencies]\nsqlx = { version = "*", default-features = false, '
            'features = ["sqlite"] }\n'
        )
        (probe / "lib.rs").touch()
        subprocess.run(
            ["cargo", "generate-lockfile", "--manifest-path", str(probe / "Cargo.toml")],
            cwd=directory,
            check=True,
            timeout=180,
        )
        available = replacement(tomllib.loads((probe / "Cargo.lock").read_text()))

    if available is None:
        print("Published SQLx still requires the SQLite dependency backport.")
        return 0

    sqlx, sqlite = available
    message = (
        f"SQLx {sqlx} supports patched libsqlite3-sys {sqlite}. "
        "Remove the sqlx-sqlite patch and vendor/sqlx-sqlite, upgrade SQLx, "
        "refresh Cargo.lock and Rust licenses, and run SQLite/PostgreSQL tests. "
        "See vendor/README.md."
    )
    print(f"::error title=Remove temporary SQLx SQLite backport::{message}")
    if summary := os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(summary, "a") as output:
            output.write(f"### SQLx backport can be removed\n\n{message}\n")
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
