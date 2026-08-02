#!/bin/sh
set -eu

test_data_dir="${TMPDIR:-/tmp}/racebin-real-stack-playwright"
password_file="$test_data_dir/password"

cleanup() {
  rm -rf "$test_data_dir"
}
trap cleanup EXIT HUP INT TERM

cleanup
mkdir -m 700 "$test_data_dir"
printf '%s\n' 'correct horse battery staple' > "$password_file"
../target/debug/racebin account create test-admin --admin \
  --password-file "$password_file" --data-dir "$test_data_dir"
rm "$password_file"

RACEBIN_BIND=127.0.0.1 \
RACEBIN_PORT=4174 \
RACEBIN_DATA_DIR="$test_data_dir" \
RACEBIN_PUBLIC_URL=http://127.0.0.1:4174 \
RACEBIN_INSECURE_COOKIE=true \
../target/debug/racebin
