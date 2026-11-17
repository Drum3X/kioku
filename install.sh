#!/usr/bin/env bash
# second-brain installer: sets up (or connects) a vault and wires it into Claude Code
# so the brain is available from every project. Safe to re-run.
#
#   ./install.sh [--vault PATH]       install or update (default vault: the one already
#                                     installed, otherwise ~/Documents/Brain)
#   ./install.sh --uninstall          remove the Claude Code wiring (the vault is kept)
set -euo pipefail
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

die() { echo "second-brain: $*" >&2; exit 1; }
warn() { echo "WARNING: $*" >&2; }

mode="install"; vault_arg=""
while [ $# -gt 0 ]; do
  case "$1" in
    --vault) vault_arg="${2:-}"; [ -n "$vault_arg" ] || die "--vault needs a path"; shift 2 ;;
    --vault=*) vault_arg="${1#--vault=}"; shift ;;
    --uninstall) mode="--uninstall"; shift ;;
    -h|--help) sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
done

# ---- 0. Requirements ----
[ "${BASH_VERSINFO[0]}" -ge 4 ] || die "bash 4 or newer is required (macOS: brew install bash)"
for c in jq rg git; do command -v "$c" >/dev/null || die "'$c' is required"; done
command -v node >/dev/null || warn "node not found: non-Latin page titles will not get a slug (Latin titles work)"

SYS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
CL="$HOME/.claude"
SETTINGS="$CL/settings.json"
UMD="$CL/CLAUDE.md"
changes=()

# Vault: --vault, else the one an earlier install wired into settings.json, else the default.
installed_vault() {
  [ -f "$SETTINGS" ] || return 0
  jq -r '.hooks.SessionStart[]?.hooks[]?.command // empty' "$SETTINGS" 2>/dev/null \
    | sed -n 's|/\.brain/hooks/session-start\.sh$||p' | head -n 1
}
OLD="$(installed_vault)"
VAULT="${vault_arg:-$OLD}"
VAULT="${VAULT:-$HOME/Documents/Brain}"
VAULT="${VAULT/#\~/$HOME}"
case "$VAULT" in /*) ;; *) VAULT="$PWD/$VAULT" ;; esac
VAULT="${VAULT%/}"
# Only one vault is wired at a time: installing another one replaces the old wiring.
[ "$OLD" = "$VAULT" ] && OLD=""

SS="$VAULT/.brain/hooks/session-start.sh"
ST="$VAULT/.brain/hooks/stop-check.sh"
today="$(date +%F)"

# ---- 1. Vault ----
if [ "$mode" != "--uninstall" ]; then
  chmod +x "$SYS/bin/brain" "$SYS"/hooks/*.sh
  [ -e "$VAULT" ] || changes+=("vault created: $VAULT")
  mkdir -p "$VAULT"

  link="$VAULT/.brain"
  if [ -L "$link" ]; then
    [ "$(cd "$link" && pwd -P)" = "$SYS" ] || die "$link points somewhere else ($(readlink "$link")); remove it or install from that checkout"
  elif [ -e "$link" ]; then
    die "$link is a real folder (older layout). Move it out of the vault, then re-run."
  else
    ln -s "$SYS" "$link"; changes+=("linked $link → $SYS")
  fi

  for d in daily journal projects raw/inbox raw/assets wiki/topics wiki/sources wiki/concepts wiki/entities wiki/ideas wiki/synthesis; do
    mkdir -p "$VAULT/$d"
    [ -n "$(ls -A "$VAULT/$d")" ] || touch "$VAULT/$d/.gitkeep"
  done

  # Skeleton files: copied only when missing, never overwritten.
  while IFS= read -r rel; do
    rel="${rel#./}"; dst="$VAULT/$rel"
    [ -e "$dst" ] && continue
    mkdir -p "$(dirname "$dst")"
    c="$(<"$SYS/skeleton/$rel")"; printf '%s\n' "${c//\{\{date\}\}/$today}" > "$dst"
    changes+=("created $rel")
  done < <(cd "$SYS/skeleton" && find . -type f | sort)

  grep -qxF '.brain' "$VAULT/.gitignore" 2>/dev/null || { printf '\n.brain\n' >> "$VAULT/.gitignore"; changes+=(".gitignore: added .brain"); }
  grep -qF '@.brain/SCHEMA.md' "$VAULT/CLAUDE.md" || warn "$VAULT/CLAUDE.md does not import the schema; add a line: @.brain/SCHEMA.md"

  if [ ! -d "$VAULT/.git" ]; then
    git -C "$VAULT" init -q
    if git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "Brain: initial vault" 2>/dev/null; then
      changes+=("git repository initialized")
    else
      warn "git init done, but the first commit failed (set git user.name / user.email, then commit)"
    fi
  fi
fi

# ---- 2. Skills (symlinks into ~/.claude/skills) ----
mkdir -p "$CL/skills"
for d in "$SYS"/skills/*/; do
  name="$(basename "$d")"; tgt="$CL/skills/$name"; src="$VAULT/.brain/skills/$name"
  if [ "$mode" = "--uninstall" ]; then
    if [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$src" ]; then rm "$tgt"; changes+=("skill removed: $name"); fi
    continue
  fi
  if [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$src" ]; then continue
  elif [ -n "$OLD" ] && [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$OLD/.brain/skills/$name" ]; then
    ln -sfn "$src" "$tgt"; changes+=("skill moved to the new vault: $name")
  elif [ -e "$tgt" ] || [ -L "$tgt" ]; then warn "$tgt already exists and does not belong to Brain; left untouched."
  else ln -s "$src" "$tgt"; changes+=("skill linked: $name")
  fi
done

# ---- 3. Block in ~/.claude/CLAUDE.md ----
block="$(cat <<EOF
<!-- brain:start — managed by second-brain/install.sh; manual edits are overwritten on the next install -->
## Brain (second brain)

The user's personal second brain is \`$VAULT\` (Obsidian vault + git). The user does not write notes by hand; you keep the records.

- At session start, the SessionStart hook adds Core.md, recent daily notes, active projects and due todos to the context.
- Before writing anything to the vault, read \`$VAULT/.brain/SCHEMA.md\` (the rules) and \`$VAULT/CLAUDE.md\` (personal rules) and follow them.
- Decisions, finished work, durable learnings, preferences, open threads → record them with the \`brain\` skill (without asking; except Core.md).
- Questions that need past knowledge → search Brain first (\`brain-ask\`). A link/PDF to "add to the brain" → \`brain-ingest\`. Maintenance → \`brain-lint\`.
- The user's own notes: timeline and todos ("today I did …", reminders) → \`brain-journal\` (journal/); durable things (tools, sites, ideas, things learned) → \`brain-note\` (wiki pages categorized in topic hubs, wiki/topics/). Don't mix them into your work log (daily/).
- Languages: system text (headings, entry types, frontmatter) is English; note content follows the language of its source or conversation.
- CLI: \`$VAULT/.brain/bin/brain\` (today, journal, todo, new, log, search, recap, lint-check).
- Built-in auto memory is only for working preferences about how to help the user; knowledge, decisions, projects and the user's profile go into Brain. Never write the same thing in both places.
<!-- brain:end -->
EOF
)"
strip_block() { awk '/<!-- brain:start/{s=1} !s{print} /<!-- brain:end -->/{s=0}' "$1"; }
if [ -f "$UMD" ]; then cur="$(<"$UMD")"; rest="$(strip_block "$UMD")"; else cur=""; rest=""; fi
rest="$(printf '%s' "$rest" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"
if [ "$mode" = "--uninstall" ]; then new="$rest"; else new="${rest:+$rest$'\n\n'}$block"; fi
if [ "$new" != "$cur" ]; then
  if [ -z "$new" ]; then rm -f "$UMD"; else printf '%s\n' "$new" > "$UMD"; fi
  changes+=("~/.claude/CLAUDE.md updated")
fi

# ---- 4. ~/.claude/settings.json (merged with jq) ----
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
jq empty "$SETTINGS" || die "$SETTINGS is not valid JSON; left untouched."
[ -f "$SETTINGS.brain.bak" ] || cp "$SETTINGS" "$SETTINGS.brain.bak"
tmp="$(mktemp)"
jq --arg ss "$SS" --arg st "$ST" --arg dir "$VAULT" --arg old "$OLD" --arg mode "$mode" '
  def drop(cmd): map(select(((.hooks // []) | map(.command) | index(cmd)) | not));
  def without(xs; ys): (xs // []) - ys;
  def rules(d): ["Read(/\(d)/**)", "Edit(/\(d)/**)"];
  ([$dir] + (if $old == "" then [] else [$old] end)) as $dirs
  | rules($dir) as $rules
  | ($dirs | map(rules(.)) | add) as $allrules
  | .hooks.SessionStart = ((.hooks.SessionStart // []) | drop($ss) | drop("\($old)/.brain/hooks/session-start.sh"))
  | .hooks.Stop         = ((.hooks.Stop // []) | drop($st) | drop("\($old)/.brain/hooks/stop-check.sh"))
  | .permissions.additionalDirectories = without(.permissions.additionalDirectories; $dirs)
  | .permissions.allow = without(.permissions.allow; $allrules)
  | .sandbox.filesystem.allowWrite = without(.sandbox.filesystem.allowWrite; $dirs)
  | if $mode == "--uninstall" then . else
      .hooks.SessionStart += [{hooks: [{type: "command", command: $ss, timeout: 10}]}]
    | .hooks.Stop         += [{hooks: [{type: "command", command: $st, timeout: 10}]}]
    | .permissions.additionalDirectories += [$dir]
    | .permissions.allow += $rules
    | .sandbox.filesystem.allowWrite += [$dir]
    end
  # Remove containers left empty
  | if (.hooks.SessionStart | length) == 0 then del(.hooks.SessionStart) else . end
  | if (.hooks.Stop | length) == 0 then del(.hooks.Stop) else . end
  | if (.hooks | length) == 0 then del(.hooks) else . end
  | if (.permissions.additionalDirectories | length) == 0 then del(.permissions.additionalDirectories) else . end
  | if (.permissions.allow | length) == 0 then del(.permissions.allow) else . end
  | if (.permissions | length) == 0 then del(.permissions) else . end
  | if (.sandbox.filesystem.allowWrite | length) == 0 then del(.sandbox.filesystem.allowWrite) else . end
  | if (.sandbox.filesystem | length) == 0 then del(.sandbox.filesystem) else . end
  | if (.sandbox | length) == 0 then del(.sandbox) else . end
' "$SETTINGS" > "$tmp"
if ! cmp -s <(jq -S . "$SETTINGS") <(jq -S . "$tmp"); then
  cat "$tmp" > "$SETTINGS"; changes+=("~/.claude/settings.json updated (backup: settings.json.brain.bak)")
fi
rm -f "$tmp"

# ---- Summary ----
if [ ${#changes[@]} -eq 0 ]; then echo "Brain: no changes (already up to date). Vault: $VAULT"
else printf 'Brain (vault: %s):\n' "$VAULT"; printf '  - %s\n' "${changes[@]}"; fi
if [ "$mode" = "--uninstall" ]; then
  echo "The vault and its .brain link were kept. Delete them yourself if you no longer want them."
else
  echo "Open a new Claude Code session; hooks and skills take effect there. Say \"get to know me\" to start."
fi
