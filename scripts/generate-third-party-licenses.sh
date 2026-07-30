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
    {
        printf '%s\n' web/node_modules/svelte
        (cd web && npm ls --omit=dev --all --parseable) | sed '1d'
    } | sort -u | while IFS= read -r package_dir; do
        package_info=$(node -e \
            'const fs=require("fs"); const p=JSON.parse(fs.readFileSync(process.argv[1]+"/package.json")); process.stdout.write(`${p.name} ${p.version}`)' \
            "$package_dir")
        license_file=
        for candidate in LICENSE LICENSE.md LICENSE.txt license LICENSE-MIT; do
            if [ -f "$package_dir/$candidate" ]; then
                license_file="$package_dir/$candidate"
                break
            fi
        done
        if [ -z "$license_file" ]; then
            printf 'No license file found for %s\n' "$package_info" >&2
            exit 1
        fi
        printf '## %s\n\n' "$package_info"
        cat "$license_file"
        printf '\n'
    done
} > "$frontend_notice"
mv "$frontend_notice" THIRD_PARTY_FRONTEND_LICENSES.md
trap - EXIT
