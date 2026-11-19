#!/usr/bin/env bash
# Kioku — Stop hook: recording safety net.
# If real work happened in the session and nothing was written to the vault, asks the agent
# ONCE per session to check whether something should be recorded.
# Disable with: KIOKU_STOP_CHECK=0
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

[ "${KIOKU_STOP_CHECK:-1}" = "0" ] && exit 0
# Vault = two levels above this script as reached through the vault's .kioku link (logical
# path, symlinks not resolved). KIOKU_DIR overrides.
VAULT="${KIOKU_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
[ -d "$VAULT" ] || exit 0
command -v jq >/dev/null || exit 0

input="$(cat 2>/dev/null || true)"
[ "$(jq -r '.stop_hook_active // false' <<<"$input" 2>/dev/null)" = "true" ] && exit 0
sid="$(jq -r '.session_id // empty' <<<"$input" 2>/dev/null)"
tp="$(jq -r '.transcript_path // empty' <<<"$input" 2>/dev/null)"
[ -n "$sid" ] && [ -f "$tp" ] || exit 0

mdir="${XDG_RUNTIME_DIR:-/tmp}/kioku-stop"
mkdir -p "$mdir" 2>/dev/null || exit 0
marker="$mdir/$sid"

# Learnings written to the daily log in this session but not into the wiki (their daily line has
# no [[wiki/…]] link yet). Asked once per session, separately from the check below.
if [ ! -e "$marker.learned" ]; then
  inputs="$(jq -r 'select(.type=="assistant") | .message.content[]? | select(.type=="tool_use") | .input | .. | strings' "$tp" 2>/dev/null)"
  yday="$(date -d yesterday +%F 2>/dev/null || date -v-1d +%F)"
  hits=""
  while IFS= read -r l; do
    txt="${l#*learned: }"
    grep -qF -- "${txt:0:40}" <<<"$inputs" && hits+="- ${txt:0:160}"$'\n'
  done < <(grep -hE '^- [0-9:]+ \[[^]]*\] learned:' "$VAULT/daily/$(date +%F).md" "$VAULT/daily/$yday.md" 2>/dev/null | grep -v '\[\[wiki/')
  if [ -n "$hits" ]; then
    : > "$marker.learned"
    jq -n --arg h "$hits" '{decision:"block", reason:("Kioku: these learnings went into the daily log in this session but not into the wiki:\n" + $h + "Following step 3 of \"Memory: capturing from conversations\" in the Kioku schema, add each durable one to its wiki/concepts or wiki/entities page (Topics line, topic hub, source link to the daily note), then end its daily line with a link to that page, e.g. ([[wiki/concepts/x]]), and commit. Mention it to the user in one line.")}'
    exit 0
  fi
fi
[ -e "$marker" ] && exit 0

# Count tool calls in the transcript: total, edits outside the vault, writes to the vault.
read -r total edits vault < <(
  jq -rc --arg b "$VAULT" '
    select(.type=="assistant") | .message.content[]? | select(.type=="tool_use")
    | [ .name, (.input.file_path // .input.notebook_path // ""), (.input.command // "") ] | @tsv
  ' "$tp" 2>/dev/null | awk -F'\t' -v b="$VAULT" -v t="${VAULT/#$HOME/\~}" '
    { total++
      wrote = 0
      if ($1 ~ /^(Edit|Write|MultiEdit|NotebookEdit)$/) {
        if (index($2, b) == 1) wrote = 1; else edits++
      }
      if ($1 == "Bash" && (index($3, b) > 0 || index($3, t) > 0) && $3 ~ /(kioku (today|journal|new|log)|commit|>>|tee )/) wrote = 1
      vault += wrote }
    END { printf "%d %d %d\n", total, edits, vault }'
)
total="${total:-0}"; edits="${edits:-0}"; vault="${vault:-0}"

# Short Q&A or already recorded: stay out of the way.
[ "$vault" -gt 0 ] && exit 0
[ "$edits" -ge 1 ] || [ "$total" -ge "${KIOKU_STOP_MIN_TOOLS:-10}" ] || exit 0

: > "$marker"
jq -n '{decision:"block", reason:"Kioku: work was done in this session but nothing was recorded to the second brain. Following \"Memory: capturing from conversations\" in the Kioku schema (.kioku/SCHEMA.md in the vault), record any durable decision, result, learning, preference or open thread and commit; otherwise say \"Kioku: nothing to record\" in one line (in the user'\''s language). Do not explain this check to the user at length."}'
