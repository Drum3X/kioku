#!/usr/bin/env bash
# Kioku — PostToolUse hook (Write|Edit|MultiEdit): checks a vault file right after it was written
# (frontmatter, Topics line, [[links]], todos/events/contacts) and tells Claude what to fix, so
# mistakes are fixed in the same turn instead of waiting for lint. Files outside the vault are
# ignored. Disable with KIOKU_WRITE_CHECK=0.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

[ "${KIOKU_WRITE_CHECK:-1}" = "0" ] && exit 0
VAULT="${KIOKU_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
command -v jq >/dev/null || exit 0
input="$(cat 2>/dev/null || true)"
f="$(jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' <<<"$input" 2>/dev/null)"
[ -n "$f" ] || exit 0
# The vault, as written or with symlinks resolved
real="$(cd "$VAULT" 2>/dev/null && pwd -P)"
case "$f" in "$VAULT"/*) ;; "$real"/*) f="$VAULT/${f#"$real"/}" ;; *) exit 0 ;; esac

out="$(KIOKU_DIR="$VAULT" "$VAULT/.kioku/bin/kioku" check-file "$f" 2>/dev/null)" && exit 0
[ -n "$out" ] || exit 0
jq -n --arg o "$out" '{decision: "block", reason: ("Kioku: the vault file you just wrote has problems:\n" + $o + "\nFix them now (see the Kioku schema). A link to a page you are about to create is fine: create that page next.")}'
