#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if grep -RInE --include='*.ts' --include='*.svelte' \
    --exclude='transport.ts' --exclude='*.test.ts' \
    'fetch[[:space:]]*\(' web/src
then
    echo "direct frontend fetch outside the API transport" >&2
    exit 1
fi

if grep -RInE --include='*.ts' --include='*.svelte' \
    '(^|[^[:alnum:]_])(requestApi|requestApiResult)([^[:alnum:]_]|$)' web/src
then
    echo "obsolete generic API request interface found" >&2
    exit 1
fi

if grep -RInE --include='*.ts' --include='*.svelte' \
    "from ['\"][^'\"]*api/(transport|normalize)['\"]" \
    web/src/pages web/src/components web/src/session.ts web/src/App.svelte
then
    echo "frontend caller bypasses the public API resource layer" >&2
    exit 1
fi
