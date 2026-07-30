#!/bin/sh
set -eu

cargo about generate licenses/about.hbs \
    --config licenses/about.toml \
    --fail \
    --locked \
    --output-file THIRD_PARTY_RUST_LICENSES.md

frontend_notice=$(mktemp)
trap 'rm -f "$frontend_notice"' EXIT
{
    printf '%s\n\n' '# Racebin third-party frontend licenses'
    printf '%s\n\n' \
        'The production browser bundle incorporates the following independently licensed packages.'
    printf '## Highlight.js %s\n\n' \
        "$(node -p "require('./web/node_modules/highlight.js/package.json').version")"
    cat web/node_modules/highlight.js/LICENSE
    printf '\n## source-map-js %s\n\n' \
        "$(node -p "require('./web/node_modules/source-map-js/package.json').version")"
    cat web/node_modules/source-map-js/LICENSE
} > "$frontend_notice"
mv "$frontend_notice" THIRD_PARTY_FRONTEND_LICENSES.md
trap - EXIT
