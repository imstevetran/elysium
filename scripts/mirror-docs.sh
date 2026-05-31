#!/usr/bin/env bash
# Mirror core/docs to the elysiumlang.github.io repository (local fallback).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOCS_REPO="${DOCS_REPO:-https://github.com/elysiumlang/elysiumlang.github.io.git}"
SOURCE="$ROOT/core/docs"

if [[ ! -d "$SOURCE" ]]; then
  echo "error: $SOURCE not found" >&2
  exit 1
fi

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

git clone --depth=1 "$DOCS_REPO" "$WORKDIR"
rsync -a --delete "$SOURCE/" "$WORKDIR/"

cd "$WORKDIR"
git add -A
if git diff --staged --quiet; then
  echo "No doc changes to mirror."
  exit 0
fi

SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo manual)"
git commit -m "docs: mirror from $(basename "$ROOT")@$SHA"
git push origin main
echo "Mirrored to $DOCS_REPO (branch main)"
