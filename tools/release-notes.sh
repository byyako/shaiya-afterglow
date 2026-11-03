#!/bin/bash
# Prints the GitHub release text of a version: its section of CHANGELOG.md and how to start.
#   tools/release-notes.sh 0.2.0
# Used by .github/workflows/release.yml.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:?usage: $0 <version, for example 0.2.0>}"
version="${version#v}"
section="$(awk -v v="$version" '
    /^## / { if (found) exit; if ($2 == v) { found = 1; next } }
    found { print }
' "$root/CHANGELOG.md")"
[ -n "$section" ] || { echo "no section for $version in CHANGELOG.md" >&2; exit 1; }
cat <<EOF
$section

---

**Try it** (Linux, with Docker and Python 3; you bring your own copy of the original EP4.5 server files):

\`\`\`bash
git clone https://github.com/byyako/shaiya-afterglow.git && cd shaiya-afterglow && git checkout v$version
tools/setup.sh /path/to/PSM_Data /path/to/PS_Game/Data
\`\`\`

Without Rust installed, the setup downloads the binaries attached to this release. The README has the details; bugs and reports go to the issues.
EOF
