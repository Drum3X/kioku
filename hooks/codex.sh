#!/usr/bin/env bash
# Kioku — hooks for OpenAI Codex (~/.codex/hooks.json).
#   codex.sh session-start   SessionStart:     the session context (session-start.sh)
#   codex.sh prompt          UserPromptSubmit: pull the vault (prompt-pull.sh, autosync only)
#   codex.sh tool            PostToolUse:      count tool calls; write check for apply_patch edits
#   codex.sh stop            Stop:             the recording reminder, then push (autosync)
# Codex sends the same input fields as Claude Code and reads the same output, so session-start
# and prompt reuse those hooks as they are. Its transcript format is not a stable interface, so
# the Stop reminder does not read it: the tool hook keeps this session's tool calls (count,
# edits outside the vault, writes to the vault, their text) in ~/.local/state/kioku/codex/.
# Never breaks a session: on any problem it prints nothing.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$HOME/.local/bin:$PATH"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -L)"
VAULT="${KIOKU_DIR:-$(cd "$here/../.." && pwd -L)}"
export KIOKU_DIR="$VAULT"
K="$VAULT/.kioku/bin/kioku"
ev="${1:-}"
input="$(cat 2>/dev/null || true)"
command -v jq >/dev/null || exit 0
jq -e . >/dev/null 2>&1 <<<"$input" || input='{}'

st="${XDG_STATE_HOME:-$HOME/.local/state}/kioku/codex"
mkdir -p "$st" 2>/dev/null || exit 0
printf '%s\n' "$input" > "$st/last-$ev.json" 2>/dev/null   # debugging; stays on this machine
sid="$(jq -r '.session_id // empty' <<<"$input" | tr -c 'A-Za-z0-9._\n-' '_')"
real="$(cd "$VAULT" 2>/dev/null && pwd -P)"
in_vault() { case "$1" in "$VAULT"|"$VAULT"/*|"$real"|"$real"/*) return 0 ;; esac; return 1; }

case "$ev" in
  session-start)
    [ -n "$sid" ] && { : >> "$st/$sid.total"; find "$st" -type f -mtime +14 -delete 2>/dev/null || true; }
    "$here/session-start.sh" <<<"$input" ;;

  prompt)
    "$here/prompt-pull.sh" <<<"$input" ;;

  tool)
    [ -n "$sid" ] || exit 0
    printf . >> "$st/$sid.total"
    tn="$(jq -r '.tool_name // empty' <<<"$input")"
    cmd="$(jq -r '.tool_input.command // .tool_input.input // .tool_input.patch // empty | if type == "array" then join(" ") else . end' <<<"$input" 2>/dev/null)"
    cwd="$(jq -r '.cwd // empty' <<<"$input")"; cwd="${cwd:-$PWD}"
    printf '%s\n' "$cmd" >> "$st/$sid.inputs"   # the learned→wiki check looks for its text here
    if [ "$tn" = Bash ]; then
      # A command that writes to the vault: run there (or naming it) and committing or appending.
      if { in_vault "$cwd" || [[ "$cmd" == *"$VAULT"* ]] || [[ "$cmd" == *"${VAULT/#$HOME/\~}"* ]]; } \
         && grep -qE 'commit|kioku (today|journal|new|log|capture)|>>|tee ' <<<"$cmd"; then : > "$st/$sid.vault"; fi
      exit 0
    fi
    [ "$tn" = apply_patch ] || exit 0
    files=()
    # File headers of the patch: "*** Add File: p", "*** Update File: p", "*** Move to: p", "*** Delete File: p"
    while IFS= read -r line; do
      f="$(sed -nE 's/^\*\*\* (Add|Update|Delete) File: (.+)$/\2/p; s/^\*\*\* Move to: (.+)$/\1/p' <<<"$line")"
      [ -n "$f" ] || continue
      case "$f" in /*) ;; *) f="$cwd/$f" ;; esac
      case "$f" in
        "$VAULT"/*) files+=("$f") ;;
        "$real"/*) files+=("$VAULT/${f#"$real"/}") ;;
        *) printf . >> "$st/$sid.edits" ;;
      esac
    done <<<"$cmd"
    [ ${#files[@]} -gt 0 ] || exit 0
    : > "$st/$sid.vault"
    [ "${KIOKU_WRITE_CHECK:-1}" = "0" ] && exit 0
    out="$("$K" check-file "${files[@]}" 2>/dev/null)" && exit 0
    [ -n "$out" ] || exit 0
    jq -n --arg o "$out" '{decision: "block", reason: ("Kioku: the vault file you just wrote has problems:\n" + $o + "\nFix them now (see the Kioku schema). A link to a page you are about to create is fine: create that page next.")}' ;;

  stop)
    # Commits made inside Codex's sandbox may not have been pushed (no network there); push now.
    ( "$K" remote now >/dev/null 2>&1 & )
    [ "${KIOKU_STOP_CHECK:-1}" = "0" ] && exit 0
    [ "$(jq -r '.stop_hook_active // false' <<<"$input")" = true ] && exit 0
    [ -n "$sid" ] && [ -f "$st/$sid.total" ] || exit 0

    # Learnings written to the daily log in this session (their text is in this session's tool
    # calls) whose line has no [[wiki/…]] link yet.
    if [ ! -e "$st/$sid.learned" ] && [ -s "$st/$sid.inputs" ]; then
      yday="$(date -d yesterday +%F 2>/dev/null || date -v-1d +%F)"
      hits=""
      while IFS= read -r l; do
        txt="${l#*learned: }"
        grep -qF -- "${txt:0:40}" "$st/$sid.inputs" && hits+="- ${txt:0:160}"$'\n'
      done < <(grep -hE '^- [0-9:]+ \[[^]]*\] learned:' "$VAULT/daily/$(date +%F).md" "$VAULT/daily/$yday.md" 2>/dev/null | grep -v '\[\[wiki/')
      if [ -n "$hits" ]; then
        : > "$st/$sid.learned"
        jq -n --arg h "$hits" '{decision:"block", reason:("Kioku: these learnings went into the daily log in this session but not into the wiki:\n" + $h + "Following step 3 of \"Memory: capturing from conversations\" in the Kioku schema, add each durable one to its wiki/concepts or wiki/entities page (Topics line, topic hub, source link to the daily note), then end its daily line with a link to that page, e.g. ([[wiki/concepts/x]]), and commit. Mention it to the user in one line.")}'
        exit 0
      fi
    fi

    [ -e "$st/$sid.asked" ] && exit 0
    [ -e "$st/$sid.vault" ] && exit 0   # something was written to the vault: recorded
    total="$(cat "$st/$sid.total" 2>/dev/null | wc -c)"; edits="$(cat "$st/$sid.edits" 2>/dev/null | wc -c)"
    [ "${edits// /}" -ge 1 ] || [ "${total// /}" -ge "${KIOKU_STOP_MIN_TOOLS:-10}" ] || exit 0
    : > "$st/$sid.asked"
    jq -n '{decision:"block", reason:"Kioku: work was done in this session but nothing was recorded to the second brain. Following \"Memory: capturing from conversations\" in the Kioku schema (.kioku/SCHEMA.md in the vault), record any durable decision, result, learning, preference or open thread and commit; otherwise say \"Kioku: nothing to record\" in one line (in the user'\''s language). Do not explain this check to the user at length."}' ;;
esac
exit 0
