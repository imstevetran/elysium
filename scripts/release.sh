#!/usr/bin/env bash
set -euo pipefail

# ──────────────────────────────────────────────────────────────────────
# Elysium release helper.
# Usage:  ./scripts/release.sh <version>
# Example: ./scripts/release.sh 0.2.0
#
# Bumps the version everywhere, commits, tags, and pushes the tag
# so the CI release workflow (.github/workflows/release.yml) takes over.
# ──────────────────────────────────────────────────────────────────────

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

if [ $# -ne 1 ]; then
  echo "Usage: $0 <version>"
  echo "Example: $0 0.2.0"
  exit 1
fi

VERSION="$1"
TAG="v$VERSION"

# Validate version format (semver)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "Error: '$VERSION' is not a valid semver (e.g. 0.2.0)"
  exit 1
fi

echo "==> Preparing release $TAG"

# ── Check working tree is clean ──
if [ -n "$(git status --porcelain)" ]; then
  echo "Error: working tree is dirty. Commit or stash changes first."
  exit 1
fi

# ── Bump version in every manifest ──
echo "==> Bumping version in Cargo.toml"
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$ROOT_DIR/Cargo.toml"

echo "==> Bumping version in npm-package/package.json"
# Use temporary file for cross-platform compatibility
node -e "
const p = require('$ROOT_DIR/npm-package/package.json');
p.version = '$VERSION';
require('fs').writeFileSync('$ROOT_DIR/npm-package/package.json', JSON.stringify(p, null, 2) + '\n');
"

echo "==> Bumping version in vscode-elysium/package.json"
node -e "
const p = require('$ROOT_DIR/vscode-elysium/package.json');
p.version = '$VERSION';
require('fs').writeFileSync('$ROOT_DIR/vscode-elysium/package.json', JSON.stringify(p, null, 2) + '\n');
"

# ── Commit and tag ──
echo "==> Committing version bump"
git add -A
git commit -m "release v$VERSION"

echo "==> Creating tag $TAG"
git tag "$TAG"

# ── Push ──
echo ""
echo "Release $TAG is ready locally."
echo "Run the following to trigger the CI release workflow:"
echo ""
echo "  git push origin main"
echo "  git push origin $TAG"
echo ""
echo "This will:"
echo "  - Build native binaries for Linux, macOS, and Windows"
echo "  - Create a GitHub Release with those binaries"
echo "  - Publish elysium-lang@$VERSION to npm"
echo "  - Deploy the docs site to GitHub Pages"
