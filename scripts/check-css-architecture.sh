#!/bin/sh
set -eu

cd "$(dirname "$0")/.."
styles=web/src/styles

if find "$styles" -type f -name '*.css' ! -name tokens.css \
    -exec grep -nH 'data-color-scheme' {} +; then
  echo "Theme selectors belong in tokens.css; components must consume semantic tokens." >&2
  exit 1
fi

if find "$styles" -type f -name '*.css' ! -name tokens.css \
    -exec grep -nHE '#[0-9a-fA-F]{3,8}|rgba?\(' {} +; then
  echo "Literal colors belong in tokens.css." >&2
  exit 1
fi

if find "$styles" -type f -name '*.css' ! -name utilities.css \
    -exec grep -nH '!important' {} +; then
  echo "!important is reserved for documented utilities." >&2
  exit 1
fi

if ! head -n 1 web/src/style.css | grep -q '^@layer reset, tokens, foundations, components, utilities, overrides;$'; then
  echo "The global cascade layer order must remain explicit and stable." >&2
  exit 1
fi

node scripts/check-css-consistency.mjs
