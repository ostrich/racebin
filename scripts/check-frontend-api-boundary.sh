#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

if rg -n 'fetch\s*\(' web/src \
    --glob '!web/src/api/transport.ts' \
    --glob '!*.test.ts'
then
    echo "direct frontend fetch outside the API transport" >&2
    exit 1
fi

if rg -n '\b(requestApi|requestApiResult)\b' web/src
then
    echo "obsolete generic API request interface found" >&2
    exit 1
fi

if rg -n "from ['\"][^'\"]*api/(transport|normalize)['\"]" \
    web/src/pages web/src/components web/src/session.ts web/src/App.svelte
then
    echo "frontend caller bypasses the public API resource layer" >&2
    exit 1
fi
