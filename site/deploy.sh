#!/usr/bin/env bash
# Build the site and rsync it to the web server.
#   deploy.sh            build, then show what would change (dry run)
#   deploy.sh --go       build, then upload for real
# The target (user@host:/path/to/shaiya/) comes from $SHAIYA_DEPLOY or the first line of site/.deploy-target (git-ignored).
# The ssh key comes from $SHAIYA_KEY or the second line of site/.deploy-target; with neither, ssh uses its own default keys.
set -euo pipefail
cd "$(dirname "$0")"

target="${SHAIYA_DEPLOY:-}"
[ -z "$target" ] && [ -f .deploy-target ] && target="$(head -n1 .deploy-target)"
[ -n "$target" ] || { echo "no target: set SHAIYA_DEPLOY or put user@host:/path/ in site/.deploy-target" >&2; exit 1; }
case "$target" in */) ;; *) target="$target/" ;; esac

key="${SHAIYA_KEY:-}"
[ -z "$key" ] && [ -f .deploy-target ] && key="$(sed -n 2p .deploy-target)"
ssh_cmd="ssh -o BatchMode=yes"
[ -z "$key" ] || ssh_cmd="ssh -i $key -o BatchMode=yes"

python3 build.py >/dev/null
[ -f dist/index.html ] || { echo "build produced no dist/index.html" >&2; exit 1; }

if [ "${1:-}" = "--go" ]; then
  rsync -az --delete --chmod=D755,F644 --human-readable --stats -e "$ssh_cmd" dist/ "$target"
else
  echo "dry run (use --go to upload) -> $target"
  rsync -azn --delete --chmod=D755,F644 --itemize-changes -e "$ssh_cmd" dist/ "$target"
fi
