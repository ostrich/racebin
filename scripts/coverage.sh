#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname "$script_dir")
cd "$repo_root"

case "${1:-all}" in
    all|rust|frontend) ;;
    *) printf 'usage: %s [all|rust|frontend]\n' "$0" >&2; exit 2 ;;
esac

if [ "${1:-all}" != frontend ]; then
    if ! cargo llvm-cov --version >/dev/null 2>&1; then
        printf '%s\n' 'cargo-llvm-cov is required for Rust coverage; install it and the matching LLVM tools first.' >&2
        exit 1
    fi
    mkdir -p target/coverage
    cargo llvm-cov --locked --all-features --lcov --output-path target/coverage/rust.lcov
fi

if [ "${1:-all}" != rust ]; then
    npm --prefix web run test:coverage
fi

printf '%s\n' 'Coverage reports are under target/coverage/.'
