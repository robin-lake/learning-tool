#!/usr/bin/env bash
# Installs learning-tool: builds the binary, registers the MCP server with Claude Code (user scope),
# and links the skills into ~/.claude/skills. Safe to run repeatedly.
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

need() {
    command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found. $2" >&2; exit 1; }
}
need cargo "Install Rust from https://rustup.rs"
need claude "Install Claude Code first."
if ! command -v pdftotext >/dev/null 2>&1; then
    echo "note: pdftotext not found; /build-topic needs it for PDFs. Install with: brew install poppler ocrmypdf"
fi

echo "==> Building and installing learning-tool"
cargo install --path crates/cli --locked

echo "==> Registering the MCP server with Claude Code (user scope)"
if claude mcp get learning-tool >/dev/null 2>&1; then
    echo "    already registered"
else
    claude mcp add --scope user learning-tool -- learning-tool mcp
fi

echo "==> Linking skills into ~/.claude/skills"
mkdir -p "$HOME/.claude/skills"
for skill in "$repo"/skills/*/; do
    name="$(basename "$skill")"
    ln -sfn "${skill%/}" "$HOME/.claude/skills/$name"
    echo "    /$name"
done

echo "==> Initializing the database"
learning-tool migrate

cat <<'MSG'

Done. Next steps:
  1. Start a new Claude Code session (so it picks up the MCP server and skills).
  2. Run /build-topic to create your first topic.
  3. Run /study each day.
  Check numbers any time with: learning-tool stats
MSG
