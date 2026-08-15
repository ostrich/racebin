#!/bin/sh
set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(dirname "$script_dir")

cd "$repo_root"
printf '%s\n' '==> Checking Rust formatting and repository naming'
cargo fmt -- --check
sh scripts/check-naming.sh

printf '%s\n' '==> Checking the frontend contract, boundaries, and CSS architecture'
cd web
npm run check:api
npm run check:api-boundary
npm run check:css
npm run check
npm run test:unit
npm run build

printf '%s\n' '==> Checking generated frontend artifacts'
cd "$repo_root"
git diff --exit-code -- web/dist openapi/openapi.json web/src/api/generated.ts
test -z "$(git ls-files --others --exclude-standard web/dist openapi/openapi.json web/src/api/generated.ts)"

printf '%s\n' '==> Running the deterministic browser gate'
cd web
npm run test:e2e:gate

printf '%s\n' 'Pre-push checks passed.'
