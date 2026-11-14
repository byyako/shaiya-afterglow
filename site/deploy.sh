#!/usr/bin/env bash
# Build the site and rsync it to the web server.
#   deploy.sh                    build, then show what would change (dry run)
#   deploy.sh --go               build, then upload for real
#   deploy.sh --go --allow-deletes   upload even when it would delete more than MAX_DELETES files on the server
# The target (user@host:/path/to/shaiya/) comes from $SHAIYA_DEPLOY or the first line of site/.deploy-target (git-ignored).
# The ssh key comes from $SHAIYA_KEY or the second line of site/.deploy-target; with neither, ssh uses its own default keys.
#
# Safety: the upload mirrors with --delete, so an empty or half-built dist/ would wipe the live site. Hence:
# - one deploy at a time (a lock on this script);
# - the build is copied to a private snapshot and rsync reads the snapshot, so another build emptying dist/ meanwhile cannot reach
#   the server;
# - the snapshot must hold every page of mkdocs.yml's nav and at least MIN_FILES files;
# - before a real upload, a dry run counts what would be deleted on the server; more than MAX_DELETES stops the upload unless
#   --allow-deletes is given.
set -euo pipefail
cd "$(dirname "$0")"

MIN_FILES="${SHAIYA_MIN_FILES:-100}"
MAX_DELETES="${SHAIYA_MAX_DELETES:-20}"

go=0 allow_deletes=0
for arg in "$@"; do
  case "$arg" in
    --go) go=1 ;;
    --allow-deletes) allow_deletes=1 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

exec 9<"$0"
flock -n 9 || { echo "another deploy is running" >&2; exit 1; }

target="${SHAIYA_DEPLOY:-}"
[ -z "$target" ] && [ -f .deploy-target ] && target="$(head -n1 .deploy-target)"
[ -n "$target" ] || { echo "no target: set SHAIYA_DEPLOY or put user@host:/path/ in site/.deploy-target" >&2; exit 1; }
case "$target" in */) ;; *) target="$target/" ;; esac

key="${SHAIYA_KEY:-}"
[ -z "$key" ] && [ -f .deploy-target ] && key="$(sed -n 2p .deploy-target)"
ssh_cmd="ssh -o BatchMode=yes"
[ -z "$key" ] || ssh_cmd="ssh -i $key -o BatchMode=yes"

python3 build.py >/dev/null

stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT
cp -a dist/. "$stage/"

# Every nav page must be in the snapshot (use_directory_urls: a/b.md -> a/b/index.html, index.md -> index.html).
python3 - "$stage" "$MIN_FILES" <<'EOF'
import os, re, sys
stage, min_files = sys.argv[1], int(sys.argv[2])
nav = open("mkdocs.yml", encoding="utf-8").read().split("\nnav:", 1)
pages = re.findall(r":\s*([\w./-]+\.md)\s*$", nav[1], re.M) if len(nav) == 2 else []
if not pages:
    sys.exit("check: no nav pages found in mkdocs.yml")
missing = []
for md in pages:
    stem = md[:-3]
    html = stem + ".html" if stem == "index" or stem.endswith("/index") else os.path.join(stem, "index.html")
    path = os.path.join(stage, html)
    if not os.path.isfile(path) or os.path.getsize(path) == 0:
        missing.append(html)
count = sum(len(files) for _, _, files in os.walk(stage))
if missing:
    sys.exit(f"check: {len(missing)} of {len(pages)} nav pages missing from the build, e.g. {', '.join(missing[:5])}")
if count < min_files:
    sys.exit(f"check: the build has {count} files, fewer than {min_files}")
print(f"check: {len(pages)} nav pages, {count} files")
EOF

rsync_opts=(-az --delete --chmod=D755,F644 -e "$ssh_cmd")
changes="$(rsync "${rsync_opts[@]}" -n --itemize-changes "$stage/" "$target")"
deletes="$(grep -c '^\*deleting' <<<"$changes" || true)"

if [ "$go" = 0 ]; then
  echo "dry run (use --go to upload) -> $target"
  printf '%s\n' "$changes"
  echo "would delete $deletes file(s) on the server"
  exit 0
fi

if [ "$deletes" -gt "$MAX_DELETES" ] && [ "$allow_deletes" = 0 ]; then
  echo "refusing: the upload would delete $deletes files on the server (limit $MAX_DELETES); check the dry run, then use --allow-deletes" >&2
  grep '^\*deleting' <<<"$changes" | head -20 >&2
  exit 1
fi

rsync "${rsync_opts[@]}" --human-readable --stats "$stage/" "$target"
