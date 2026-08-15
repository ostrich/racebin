#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
styles=web/src/styles

if rg -n --glob '!tokens.css' 'data-color-scheme' "$styles"; then
  echo "Theme selectors belong in tokens.css; components must consume semantic tokens." >&2
  exit 1
fi

if rg -n --glob '!tokens.css' '#[0-9a-fA-F]{3,8}|rgba?\(' "$styles"; then
  echo "Literal colors belong in tokens.css." >&2
  exit 1
fi

if rg -n --glob '!utilities.css' '!important' "$styles"; then
  echo "!important is reserved for documented utilities." >&2
  exit 1
fi

if ! head -n 1 web/src/style.css | grep -q '^@layer reset, tokens, foundations, components, utilities, overrides;$'; then
  echo "The global cascade layer order must remain explicit and stable." >&2
  exit 1
fi

node scripts/check-css-consistency.mjs
