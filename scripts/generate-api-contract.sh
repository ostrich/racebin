#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname "$script_dir")
contract_output=${1:-"$repo_root/openapi/openapi.json"}
types_output=${2:-"$repo_root/web/src/api/generated.ts"}
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

mkdir -p "$(dirname "$contract_output")" "$(dirname "$types_output")"
cargo run --quiet --locked --manifest-path "$repo_root/Cargo.toml" -- openapi \
  > "$temporary_dir/openapi.json"
node "$script_dir/normalize-openapi.mjs" "$temporary_dir/openapi.json" "$contract_output"
(
  cd "$repo_root/web"
  npm exec openapi-typescript -- "$contract_output" -o "$types_output"
)
