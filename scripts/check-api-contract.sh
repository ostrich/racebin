#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname "$script_dir")
temporary_dir=$(mktemp -d)
trap 'rm -rf "$temporary_dir"' EXIT HUP INT TERM

generator="$script_dir/generate-api-contract.sh"
local_generator="$repo_root/local/generate-api-contract.sh"
if [ -x "$local_generator" ]; then
  generator=$local_generator
fi

"$generator" \
  "$temporary_dir/openapi.json" "$temporary_dir/generated.ts"
diff -u "$repo_root/openapi/openapi.json" "$temporary_dir/openapi.json"
diff -u "$repo_root/web/src/api/generated.ts" "$temporary_dir/generated.ts"
