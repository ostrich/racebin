#!/bin/sh

set -eu

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
classifier="$script_dir/classify-ci-paths.sh"

assert_profile() {
    description=$1
    expected=$2
    shift 2
    actual=$($classifier "$@")
    if [ "$actual" != "$expected" ]; then
        printf 'CI classification failed: %s\nexpected:\n%s\nactual:\n%s\n' \
            "$description" "$expected" "$actual" >&2
        exit 1
    fi
}

none='backend=false
frontend=false
api=false
real_stack=false
visual=false'
backend='backend=true
frontend=false
api=false
real_stack=false
visual=false'
frontend='backend=false
frontend=true
api=false
real_stack=false
visual=true'
api_client='backend=false
frontend=true
api=true
real_stack=true
visual=false'
contract='backend=true
frontend=true
api=true
real_stack=true
visual=false'
combined='backend=true
frontend=true
api=true
real_stack=false
visual=true'
full='backend=true
frontend=true
api=true
real_stack=true
visual=true'

assert_profile 'documentation only' "$none" docs/architecture.md CONTRIBUTING.md
assert_profile 'generated package metadata only' "$none" packaging/arch/PKGBUILD
assert_profile 'packaged service definition' "$backend" packaging/racebin.service
assert_profile 'database migration' "$backend" migrations/postgres/001_initial.sql
assert_profile 'backend domain change' "$backend" src/pastes/operations.rs
assert_profile 'frontend component change' "$frontend" web/src/pages/PasteView.svelte
assert_profile 'frontend API client change' "$api_client" web/src/api/pastes.ts
assert_profile 'HTTP contract change' "$contract" src/http/contract/pastes.rs
assert_profile 'workflow change' "$full" .github/workflows/ci.yml
assert_profile 'unknown area defaults to full coverage' "$full" tools/new-checker.rs
assert_profile 'combined areas accumulate coverage' "$combined" \
    src/pastes/operations.rs web/src/styles/paste-view.css openapi/openapi.json

printf '%s\n' 'CI path classification tests passed.'
