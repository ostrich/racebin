#!/bin/sh
set -eu

pattern='\b(pasta|pasta_file|app_user|user_session|user_invite|slug|owner_user_id|burn_after_reads|force_password_change|paste:admin|user:admin|invite:admin|invitation:admin|key:admin|RACEBIN_TITLE|RACEBIN_NO_FILE_UPLOAD|RACEBIN_MAX_FILE_SIZE_MB|RACEBIN_QR)\b'

if rg -n -i \
    --glob '!web/dist/**' \
    --glob '!arch/src/**' \
    --glob '!arch/pkg/**' \
    --glob '!target/**' \
    --glob '!scripts/check-naming.sh' \
    "$pattern" .
then
    echo "retired naming found" >&2
    exit 1
fi
