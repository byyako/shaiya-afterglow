#!/bin/bash
# Apply the operator's edits to the loaded `defs` schema (`data-editor apply`, docs/plans/done/data-editing.md section 3), and with
# `--with-additions` first the server's own definition rows (tools/defs/additions.sql: the Kill Boost Stones, items 100/212-214 and
# skill 200/1-3; not in the original, so not loaded by default: docs/server/deviations.md section 1). An edit wins over an addition on the
# same field. Run after every load_into_postgres.py and its verify.py; restart the game server afterwards (it reads `defs` at start).
#   tools/defs/additions.sh [--with-additions]
set -eu
root="$(cd "$(dirname "$0")/../.." && pwd)"
. "$root/tools/dbconn.sh"
with_additions=0
for arg in "$@"; do
    case "$arg" in
        --with-additions) with_additions=1 ;;
        *) echo "usage: $0 [--with-additions]" >&2; exit 2 ;;
    esac
done
if [ "$with_additions" = 1 ]; then
    psql_cmd < "$root/tools/defs/additions.sql"
    echo "defs additions applied"
else
    echo "defs additions skipped (not in the original); add them with: $0 --with-additions"
fi
if [ -x "$root/target/release/data-editor" ]; then
    (cd "$root" && target/release/data-editor apply)
else
    echo "data-editor is not built: run 'cargo build --release -p data-editor' and then 'target/release/data-editor apply' to write the edits back"
fi
