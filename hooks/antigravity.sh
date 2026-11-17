#!/usr/bin/env bash
# Kioku — hooks for Google Antigravity (~/.gemini/config/hooks.json).
#   antigravity.sh session-start   SessionStart:  the session context (session-start.sh)
#   antigravity.sh pull            PreInvocation: pull the vault (prompt-pull.sh, autosync only)
#   antigravity.sh stop            Stop:          the recording reminder (stop-check.sh)
# An adapter only: it turns Antigravity's input (camelCase, its own transcript) into what the
# Claude Code hooks read, and their answers into Antigravity's output. Never breaks a session:
# on any problem it prints the "do nothing" answer.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -L)"
VAULT="${KIOKU_DIR:-$(cd "$here/../.." && pwd -L)}"
export KIOKU_DIR="$VAULT"
ev="${1:-}"
input="$(cat 2>/dev/null || true)"

noop() { if [ "$ev" = stop ]; then echo '{"decision":"allow"}'; else echo '{}'; fi; exit 0; }
command -v jq >/dev/null || noop
jq -e . >/dev/null 2>&1 <<<"$input" || input='{}'

# The last input of each event, to see what Antigravity sends (debugging; stays on this machine).
st="${XDG_STATE_HOME:-$HOME/.local/state}/kioku"
mkdir -p "$st" 2>/dev/null && printf '%s\n' "$input" > "$st/antigravity-$ev.json" 2>/dev/null

# A Claude Code hook's additionalContext → an Antigravity system message.
inject() {
  local c; c="$(jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null)"
  [ -n "$c" ] || noop
  jq -n --arg c "$c" '{injectSteps: [{systemMessage: {systemMessage: $c}}]}'
}

case "$ev" in
  session-start)
    cwd="$(jq -r '.workspacePaths[0] // empty' <<<"$input")"
    jq -n --arg cwd "${cwd:-$PWD}" '{cwd: $cwd}' | "$here/session-start.sh" | inject ;;

  pull)
    echo '{}' | "$here/prompt-pull.sh" | inject ;;

  stop)
    # Only a normal end of a turn (NO_TOOL_CALL / model_stop), not an error or the step limit;
    # and only when the agent is really done (fullyIdle).
    jq -e '(.terminationReason // "" | ascii_downcase | test("error|max_steps|exceeded|cancel") | not) and (.fullyIdle != false)' <<<"$input" >/dev/null || noop
    sid="$(jq -r '.conversationId // empty' <<<"$input")"
    tp="$(jq -r '.transcriptPath // empty' <<<"$input")"
    [ -n "$sid" ] && [ -f "$tp" ] || noop
    # Antigravity transcript (one step per line, tool args as JSON strings) → the Claude Code
    # shape stop-check.sh reads. File writes become Edit, commands become Bash.
    tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
    jq -c '
      select(.tool_calls) | .tool_calls[]
      | (.args // {} | with_entries(.value |= (. as $v | try fromjson catch $v))) as $a
      | (.name // "") as $n
      # All args are kept: the learned→wiki check looks for its text in any of them.
      | (if $n == "run_command" then {name: "Bash", input: ($a + {command: ($a.CommandLine // "")})}
         elif ($n | test("write|edit|replace|create_file"; "i")) then
           {name: "Edit", input: ($a + {file_path: ($a.Path // $a.TargetFile // $a.AbsolutePath // $a.FilePath // "")})}
         else {name: $n, input: $a} end) as $t
      | {type: "assistant", message: {content: [{type: "tool_use", name: $t.name, input: $t.input}]}}
    ' "$tp" > "$tmp" 2>/dev/null || noop
    out="$(jq -n --arg s "ag-$sid" --arg t "$tmp" '{session_id: $s, transcript_path: $t}' | "$here/stop-check.sh")"
    reason="$(jq -r 'select(.decision == "block") | .reason // empty' <<<"$out" 2>/dev/null)"
    [ -n "$reason" ] || noop
    jq -n --arg r "$reason" '{decision: "continue", reason: $r}' ;;

  *) noop ;;
esac
