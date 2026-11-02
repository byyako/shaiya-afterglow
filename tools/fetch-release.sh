#!/bin/bash
# Downloads the Linux binaries of a release into target/release/, so that nothing has to be built:
#   tools/fetch-release.sh [tag]        # no tag: the latest release
# The binaries are the ones tools/setup.sh and the run scripts look for. The checksum of the download is checked.
# The release is built by .github/workflows/release.yml when a version tag is pushed.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
repo="byyako/shaiya-afterglow"
for tool in curl tar sha256sum python3; do
    command -v "$tool" > /dev/null || { echo "missing: $tool" >&2; exit 1; }
done
[ "$(uname -m)" = x86_64 ] && [ "$(uname -s)" = Linux ] || { echo "the releases are built for Linux on x86-64 only; build from source (cargo build --release --workspace)" >&2; exit 1; }

tag="${1:-}"
if [ -z "$tag" ]; then
    api="https://api.github.com/repos/$repo/releases/latest"
else
    api="https://api.github.com/repos/$repo/releases/tags/$tag"
fi
json="$(curl -fsSL -H 'Accept: application/vnd.github+json' "$api")" \
    || { echo "no release found ($api): build from source with cargo build --release --workspace" >&2; exit 1; }
tag="$(echo "$json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
asset="shaiya-afterglow-$tag-linux-x86_64.tar.gz"
base="https://github.com/$repo/releases/download/$tag"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
echo "downloading $asset"
curl -fSL -o "$work/$asset" "$base/$asset"
curl -fsSL -o "$work/$asset.sha256" "$base/$asset.sha256"
(cd "$work" && sha256sum -c "$asset.sha256")
tar -xzf "$work/$asset" -C "$work"
mkdir -p "$root/target/release"
install -m 755 "$work/shaiya-afterglow-$tag-linux-x86_64"/* "$root/target/release/"
echo "installed into target/release/: $(ls "$work/shaiya-afterglow-$tag-linux-x86_64" | tr '\n' ' ')"
echo "note: the release matches its own source; if you cloned a different commit, run 'git checkout $tag' so the database migrations agree"
