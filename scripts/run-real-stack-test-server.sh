#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname "$script_dir")
racebin_binary="$repo_root/target/debug/racebin"
test_data_dir="${TMPDIR:-/tmp}/racebin-real-stack-playwright"
password_file="$test_data_dir/password"

cleanup() {
  rm -rf "$test_data_dir"
}
trap cleanup EXIT HUP INT TERM

cleanup
mkdir -m 700 "$test_data_dir"
printf '%s\n' 'correct horse battery staple' > "$password_file"
if [ "${RACEBIN_REAL_STACK_PREBUILT:-false}" = true ]; then
  if [ ! -x "$racebin_binary" ]; then
    printf '%s\n' "prebuilt real-stack binary not found: $racebin_binary" >&2
    exit 1
  fi
else
  npm --prefix "$repo_root/web" run build
  cargo build --manifest-path "$repo_root/Cargo.toml" --locked
fi
"$racebin_binary" account create test-admin --admin \
  --password-file "$password_file" --data-dir "$test_data_dir"
rm "$password_file"

RACEBIN_BIND=127.0.0.1 \
RACEBIN_PORT=4174 \
RACEBIN_DATA_DIR="$test_data_dir" \
RACEBIN_PUBLIC_URL=http://127.0.0.1:4174 \
RACEBIN_INSECURE_COOKIE=true \
"$racebin_binary"
