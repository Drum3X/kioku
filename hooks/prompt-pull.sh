#!/usr/bin/env bash
# Kioku — UserPromptSubmit hook: with autosync on, pulls the vault before a prompt (at most once
# every 30 s), so notes written on another machine (e.g. the phone via a server) show up in a
# session that was already open. If something came in, Claude is told which files changed.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

VAULT="${KIOKU_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
[ -d "$VAULT/.git" ] || exit 0
[ "$(git -C "$VAULT" config --bool kioku.autosync 2>/dev/null)" = true ] || exit 0
cat >/dev/null 2>&1 || true

st="${XDG_STATE_HOME:-$HOME/.local/state}/kioku"
mkdir -p "$st" 2>/dev/null || exit 0
stamp="$st/prompt-pull.stamp" now="$(date +%s)"
last="$(cat "$stamp" 2>/dev/null || echo 0)"
[ $((now - last)) -lt 30 ] && exit 0
echo "$now" > "$stamp"

g=(git -C "$VAULT")
# What this copy already had from the remote (a git fetch elsewhere may have moved @{u} already)
old="$("${g[@]}" merge-base HEAD '@{u}' 2>/dev/null)" || exit 0
"$VAULT/.kioku/bin/kioku" remote pull >/dev/null 2>&1 || exit 0
new="$("${g[@]}" rev-parse -q --verify '@{u}' 2>/dev/null)" || exit 0
[ "$old" = "$new" ] && exit 0

commits="$("${g[@]}" log --no-merges --format='- %s' "$old..$new" 2>/dev/null | head -n 10)"
files="$("${g[@]}" diff --name-only "$old" "$new" 2>/dev/null | head -n 20 | sed 's/^/- /')"
msg="The vault was just updated from its git remote (written on another machine):
$commits
Changed files:
$files
Anything about these (e.g. the todos, agenda or recent notes in the session-start context) is out of date: read the files again before answering."
if command -v jq >/dev/null; then
  jq -n --arg c "$msg" '{hookSpecificOutput: {hookEventName: "UserPromptSubmit", additionalContext: $c}}'
else
  printf '%s\n' "$msg"
fi
