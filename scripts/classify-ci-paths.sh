#!/bin/sh

set -eu

backend=false
frontend=false
api=false
real_stack=false
visual=false

enable_backend() {
    backend=true
}

enable_frontend() {
    frontend=true
}

enable_api() {
    frontend=true
    api=true
}

enable_real_stack() {
    frontend=true
    real_stack=true
}

enable_visual() {
    frontend=true
    visual=true
}

enable_all() {
    backend=true
    frontend=true
    api=true
    real_stack=true
    visual=true
}

classify_path() {
    path=$1
    case "$path" in
        README.md|CONTRIBUTING.md|LICENSE|THIRD_PARTY_*.md|docs/*|packaging/arch/PKGBUILD|.gitignore)
            ;;
        packaging/*)
            enable_backend
            ;;
        .github/workflows/*|scripts/classify-ci-paths.sh|scripts/test-classify-ci-paths.sh)
            enable_all
            ;;
        Cargo.toml|Cargo.lock|build.rs|migrations/*|src/database/*|src/accounts/*|src/instance/*|src/pastes/*|src/cli/*|tests/*)
            enable_backend
            ;;
        src/http/*)
            enable_backend
            enable_api
            enable_real_stack
            ;;
        src/*)
            enable_backend
            enable_real_stack
            ;;
        openapi/*|scripts/check-api-contract.sh|scripts/generate-api-contract.sh|scripts/normalize-openapi.mjs)
            enable_api
            ;;
        scripts/check-naming.sh|scripts/generate-third-party-licenses.sh)
            enable_backend
            ;;
        scripts/check-before-push.sh|scripts/check-css-architecture.sh|scripts/check-css-consistency.mjs|scripts/check-frontend-api-boundary.sh|scripts/run-real-stack-test-server.sh|scripts/summarize-playwright-results.mjs)
            enable_all
            ;;
        web/src/api/*)
            enable_api
            enable_real_stack
            ;;
        web/src/*)
            enable_frontend
            enable_visual
            ;;
        web/e2e/visual-regression.spec.ts|web/e2e/visual-regression.spec.ts-snapshots/*)
            enable_visual
            ;;
        web/e2e/real/*)
            enable_frontend
            enable_real_stack
            ;;
        web/e2e/*|web/package.json|web/package-lock.json|web/index.html|web/playwright*.ts|web/tsconfig.json|web/vite.config.*|web/vitest.config.*|web/dist/*)
            enable_frontend
            enable_visual
            ;;
        web/*)
            enable_frontend
            ;;
        *)
            # New or unfamiliar source areas receive complete coverage until
            # their ownership is made explicit here.
            enable_all
            ;;
    esac
}

if [ "$#" -gt 0 ]; then
    for path do
        classify_path "$path"
    done
else
    while IFS= read -r path; do
        [ -n "$path" ] && classify_path "$path"
    done
fi

printf 'backend=%s\n' "$backend"
printf 'frontend=%s\n' "$frontend"
printf 'api=%s\n' "$api"
printf 'real_stack=%s\n' "$real_stack"
printf 'visual=%s\n' "$visual"
