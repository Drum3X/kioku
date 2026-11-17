#!/usr/bin/env bash
# kioku installer: sets up (or connects) a vault and wires it into Claude Code (and Google
# Antigravity, when present)
# so the brain is available from every project. Safe to re-run.
#
#   ./install.sh [--vault PATH]       install or update (default vault: the one already
#                                     installed, otherwise ~/Documents/Kioku)
#   ./install.sh --uninstall          remove the Claude Code wiring (the vault is kept)
#
#   --relink        re-point an existing .kioku link at this copy (used by the Nix module,
#                   whose store path changes on every update)
#   --no-bin-link   don't link the kioku command into ~/.local/bin (a package provides it)
#   --no-antigravity  don't set Kioku up in Google Antigravity (done when ~/.gemini exists)
set -euo pipefail
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"

die() { echo "kioku: $*" >&2; exit 1; }
warn() { echo "WARNING: $*" >&2; }

mode="install"; vault_arg=""; relink=""; bin_link=1; antigravity=1
while [ $# -gt 0 ]; do
  case "$1" in
    --vault) vault_arg="${2:-}"; [ -n "$vault_arg" ] || die "--vault needs a path"; shift 2 ;;
    --vault=*) vault_arg="${1#--vault=}"; shift ;;
    --uninstall) mode="--uninstall"; shift ;;
    --relink) relink=1; shift ;;
    --no-bin-link) bin_link=""; shift ;;
    --no-antigravity) antigravity=""; shift ;;
    -h|--help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
    | sed -nE 's#/\.(kioku|brain)/hooks/session-start\.sh$##p' | head -n 1
}
OLD="$(installed_vault)"
VAULT="${vault_arg:-$OLD}"
VAULT="${VAULT:-$HOME/Documents/Kioku}"
VAULT="${VAULT/#\~/$HOME}"
case "$VAULT" in /*) ;; *) VAULT="$PWD/$VAULT" ;; esac
VAULT="${VAULT%/}"
# Only one vault is wired at a time: installing another one replaces the old wiring.
[ "$OLD" = "$VAULT" ] && OLD=""

# Earlier versions were called second-brain (vault link .brain, CLI brain, skills brain-*).
migrate_legacy() {
  local c="${XDG_CONFIG_HOME:-$HOME/.config}" s="${XDG_STATE_HOME:-$HOME/.local/state}" d
  for d in "$c" "$s"; do
    if [ -d "$d/second-brain" ] && [ ! -e "$d/kioku" ]; then mv "$d/second-brain" "$d/kioku"; changes+=("moved $d/second-brain → $d/kioku"); fi
  done
  if [ -L "$VAULT/.brain" ]; then rm "$VAULT/.brain"; changes+=("removed the old .brain link"); fi
  if [ -f "$VAULT/CLAUDE.md" ] && grep -qF '@.brain/SCHEMA.md' "$VAULT/CLAUDE.md"; then
    sed -i.bak 's|@\.brain/SCHEMA\.md|@.kioku/SCHEMA.md|' "$VAULT/CLAUDE.md" && rm -f "$VAULT/CLAUDE.md.bak"; changes+=("CLAUDE.md: schema import now @.kioku/SCHEMA.md")
  fi
  if [ -d "$VAULT/.git" ] && [ "$(git -C "$VAULT" config --bool brain.autosync 2>/dev/null)" = true ]; then
    git -C "$VAULT" config kioku.autosync true; git -C "$VAULT" config --unset brain.autosync; changes+=("autosync setting moved to kioku.autosync")
  fi
  local l
  for l in "$CL"/skills/brain "$CL"/skills/brain-*; do
    [ -L "$l" ] && case "$(readlink "$l")" in */.brain/skills/*) rm "$l"; changes+=("old skill link removed: ${l##*/}") ;; esac
  done
  return 0
}

SS="$VAULT/.kioku/hooks/session-start.sh"
ST="$VAULT/.kioku/hooks/stop-check.sh"
UP="$VAULT/.kioku/hooks/prompt-pull.sh"
today="$(date +%F)"

# ---- 1. Vault ----
if [ "$mode" != "--uninstall" ]; then
  chmod +x "$SYS/bin/kioku" "$SYS/bin/kioku-version" "$SYS"/hooks/*.sh 2>/dev/null || true   # read-only in the Nix store
  [ -e "$VAULT" ] || changes+=("vault created: $VAULT")
  mkdir -p "$VAULT"

  migrate_legacy
  link="$VAULT/.kioku"
  if [ -L "$link" ]; then
    if [ "$(cd "$link" 2>/dev/null && pwd -P)" != "$SYS" ]; then
      [ -n "$relink" ] || die "$link points somewhere else ($(readlink "$link")); remove it or install from that checkout (or pass --relink)"
      ln -sfn "$SYS" "$link"; changes+=("relinked $link → $SYS")
    fi
  elif [ -e "$link" ]; then
    die "$link is a real folder (older layout). Move it out of the vault, then re-run."
  else
    ln -s "$SYS" "$link"; changes+=("linked $link → $SYS")
  fi

  for d in daily journal calendar contacts projects raw/inbox raw/assets wiki/topics wiki/sources wiki/concepts wiki/entities wiki/ideas wiki/synthesis; do
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

  grep -qxF '.kioku' "$VAULT/.gitignore" 2>/dev/null || { printf '\n.kioku\n' >> "$VAULT/.gitignore"; changes+=(".gitignore: added .kioku"); }
  grep -qF '@.kioku/SCHEMA.md' "$VAULT/CLAUDE.md" || warn "$VAULT/CLAUDE.md does not import the schema; add a line: @.kioku/SCHEMA.md"

  if [ ! -d "$VAULT/.git" ]; then
    git -C "$VAULT" init -q
    if git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "Kioku: initial vault" 2>/dev/null; then
      changes+=("git repository initialized")
    else
      warn "git init done, but the first commit failed (set git user.name / user.email, then commit)"
    fi
  fi

  # Vault git hooks: stable ids for todos/events/contacts, background sync after commits
  hk="$("$VAULT/.kioku/bin/kioku" sync --setup 2>&1)" || warn "git hooks: $hk"
  [ -n "$hk" ] && changes+=("git hooks: ${hk//$'\n'/, }")
fi

# ---- 1b. The kioku command on PATH (~/.local/bin/kioku → the vault's CLI) ----
BIN="$HOME/.local/bin"; cli="$BIN/kioku"; cli_src="$VAULT/.kioku/bin/kioku"
if [ "$mode" = "--uninstall" ] || [ -z "$bin_link" ]; then
  if [ -L "$cli" ] && [ "$(readlink "$cli")" = "$cli_src" ]; then rm "$cli"; changes+=("removed $cli"); fi
else
  if [ -L "$cli" ] && [ "$(readlink "$cli")" = "$cli_src" ]; then :
  elif [ -L "$cli" ] && [[ "$(readlink "$cli")" == */.kioku/bin/kioku ]]; then ln -sfn "$cli_src" "$cli"; changes+=("$cli now points to this vault")
  elif [ -e "$cli" ] || [ -L "$cli" ]; then warn "$cli already exists and is not ours; left untouched"
  else mkdir -p "$BIN"; ln -s "$cli_src" "$cli"; changes+=("command linked: $cli")
  fi
  case ":$PATH:" in *":$BIN:"*) ;; *) warn "$BIN is not on your PATH; add it to use the kioku command (NixOS/home-manager: home.sessionPath = [ \"\$HOME/.local/bin\" ];)" ;; esac
fi

# ---- 2. Skills (symlinks into ~/.claude/skills) ----
mkdir -p "$CL/skills"
for d in "$SYS"/skills/*/; do
  name="$(basename "$d")"; tgt="$CL/skills/$name"; src="$VAULT/.kioku/skills/$name"
  if [ "$mode" = "--uninstall" ]; then
    if [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$src" ]; then rm "$tgt"; changes+=("skill removed: $name"); fi
    continue
  fi
  if [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$src" ]; then continue
  elif [ -n "$OLD" ] && [ -L "$tgt" ] && [ "$(readlink "$tgt")" = "$OLD/.kioku/skills/$name" ]; then
    ln -sfn "$src" "$tgt"; changes+=("skill moved to the new vault: $name")
  elif [ -e "$tgt" ] || [ -L "$tgt" ]; then warn "$tgt already exists and does not belong to Kioku; left untouched."
  else ln -s "$src" "$tgt"; changes+=("skill linked: $name")
  fi
done

# ---- 3. Block in ~/.claude/CLAUDE.md ----
block="$(cat <<EOF
<!-- kioku:start — managed by kioku/install.sh; manual edits are overwritten on the next install -->
## Kioku (second brain)

The user's personal second brain is \`$VAULT\` (Obsidian vault + git). The user does not write notes by hand; you keep the records.

- At session start, the SessionStart hook adds Core.md, recent daily notes, active projects and due todos to the context.
- Before writing anything to the vault, read \`$VAULT/.kioku/SCHEMA.md\` (the rules) and \`$VAULT/CLAUDE.md\` (personal rules) and follow them.
- Decisions, finished work, durable learnings, preferences, open threads → record them with the \`kioku\` skill (without asking; except Core.md).
- Questions that need past knowledge → search Kioku first (\`kioku-ask\`). A link/PDF to "add to the brain" → \`kioku-ingest\`. Maintenance → \`kioku-lint\`.
- The user's own notes: timeline and todos ("today I did …", reminders) → \`kioku-journal\` (journal/); durable things (tools, sites, ideas, things learned) → \`kioku-note\` (wiki pages categorized in topic hubs, wiki/topics/). Don't mix them into your work log (daily/).
- Events and appointments → \`kioku-calendar\` (calendar/); contact details → \`kioku-contacts\` (contacts/). Both sync one-way to the user's CalDAV/CardDAV server after each commit; never handle the password.
- Languages: system text (headings, entry types, frontmatter) is English; note content follows the language of its source or conversation.
- CLI: \`$VAULT/.kioku/bin/kioku\` (today, journal, todo, agenda, new, log, search, recap, sync, lint-check).
- Built-in auto memory is only for working preferences about how to help the user; knowledge, decisions, projects and the user's profile go into Kioku. Never write the same thing in both places.
<!-- kioku:end -->
EOF
)"
strip_block() { awk '/<!-- (kioku|brain):start/{s=1} !s{print} /<!-- (kioku|brain):end -->/{s=0}' "$1"; }
if [ -f "$UMD" ]; then cur="$(<"$UMD")"; rest="$(strip_block "$UMD")"; else cur=""; rest=""; fi
rest="$(printf '%s' "$rest" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"
if [ "$mode" = "--uninstall" ]; then new="$rest"; else new="${rest:+$rest$'\n\n'}$block"; fi
if [ "$new" != "$cur" ]; then
  if [ -z "$new" ]; then rm -f "$UMD"; else printf '%s\n' "$new" > "$UMD"; fi
  changes+=("~/.claude/CLAUDE.md updated")
fi

# ---- 3b. Antigravity (Google): rules block in ~/.gemini/GEMINI.md, skills in ~/.gemini/config/skills,
# hooks in ~/.gemini/config/hooks.json (through hooks/antigravity.sh). Set up when Antigravity is
# present; --no-antigravity removes it again.
GEM="$HOME/.gemini"; GMD="$GEM/GEMINI.md"; GSK="$GEM/config/skills"
ag=""; if [ "$mode" != "--uninstall" ] && [ -n "$antigravity" ] && { [ -d "$GEM" ] || command -v antigravity >/dev/null; }; then ag=1; fi
gblock="$(cat <<EOF
<!-- kioku:start — managed by kioku/install.sh; manual edits are overwritten on the next install -->
## Kioku (second brain)

The user's personal second brain is \`$VAULT\` (Obsidian vault + git). The user does not write notes by hand; you keep the records.

- **First thing in every conversation:** if the conversation does not already contain "# Kioku — session context" (the SessionStart hook adds it), run \`$VAULT/.kioku/bin/kioku context\` and read its output before you answer. It pulls the vault from its git remote and prints Core.md (who the user is, how to address them, their preferences), recent daily notes, active projects, due todos and upcoming events. Follow Core.md.
- That context is a snapshot. Before answering about todos, the agenda or recent notes later in a conversation, run \`kioku todo --soon\`, \`kioku agenda\` or read the files again. When a message says "The vault was just updated from its git remote", re-read the files it lists.
- Before writing anything to the vault, read \`$VAULT/.kioku/SCHEMA.md\` (the rules) and \`$VAULT/CLAUDE.md\` (personal rules; the file is named for Claude Code but applies to you too) and follow them.
- Decisions, finished work, durable learnings, preferences, open threads → record them with the \`kioku\` skill (without asking; except Core.md). Before your final answer on a task, check yourself whether something should be recorded (a Stop hook reminds you once if nothing was).
- Questions that need past knowledge → search Kioku first (\`kioku-ask\`). A link/PDF to "add to the brain" → \`kioku-ingest\`. Maintenance → \`kioku-lint\`.
- The user's own notes: timeline and todos ("today I did …", reminders) → \`kioku-journal\` (journal/); durable things (tools, sites, ideas, things learned) → \`kioku-note\` (wiki pages categorized in topic hubs, wiki/topics/). Don't mix them into your work log (daily/).
- Events and appointments → \`kioku-calendar\` (calendar/); contact details → \`kioku-contacts\` (contacts/). Both sync one-way to the user's CalDAV/CardDAV server after each commit; never handle the password.
- Languages: system text (headings, entry types, frontmatter) is English; note content follows the language of its source or conversation.
- CLI: \`$VAULT/.kioku/bin/kioku\` (context, today, journal, todo, agenda, new, log, search, recap, sync, lint-check).
- Your built-in memory (Knowledge items) is only for working preferences about how to help the user; knowledge, decisions, projects and the user's profile go into Kioku. Never write the same thing in both places.
<!-- kioku:end -->
EOF
)"
if [ -n "$ag" ] || [ -f "$GMD" ]; then
  if [ -f "$GMD" ]; then cur="$(<"$GMD")"; rest="$(strip_block "$GMD")"; else cur=""; rest=""; fi
  rest="$(printf '%s' "$rest" | sed -e :a -e '/^\n*$/{$d;N;ba' -e '}')"
  if [ -n "$ag" ]; then new="${rest:+$rest$'\n\n'}$gblock"; else new="$rest"; fi
  if [ "$new" != "$cur" ]; then
    mkdir -p "$GEM"
    if [ -z "$new" ]; then rm -f "$GMD"; else printf '%s\n' "$new" > "$GMD"; fi
    changes+=("~/.gemini/GEMINI.md updated")
  fi
fi
# Hooks: merged into hooks.json under kioku-* names; other hooks there are kept.
GHK="$GEM/config/hooks.json"; AH="$VAULT/.kioku/hooks/antigravity.sh"
if [ -n "$ag" ] || [ -f "$GHK" ]; then
  [ -f "$GHK" ] && { jq empty "$GHK" 2>/dev/null || { warn "$GHK is not valid JSON; Antigravity hooks not set up"; GHK=""; }; }
  if [ -n "$GHK" ]; then
    hcur="$( [ -f "$GHK" ] && cat "$GHK" || echo '{}')"
    hnew="$(jq --arg ah "$AH" --arg on "$ag" '
      with_entries(select(.key | startswith("kioku-") | not))
      | if $on == "" then . else . + {
          "kioku-session-start": {SessionStart: [{type: "command", command: "\($ah) session-start", timeout: 15}]},
          "kioku-pull":          {PreInvocation: [{type: "command", command: "\($ah) pull", timeout: 30}]},
          "kioku-stop":          {Stop: [{type: "command", command: "\($ah) stop", timeout: 15}]}
        } end' <<<"$hcur")"
    if ! cmp -s <(jq -S . <<<"$hcur") <(jq -S . <<<"$hnew"); then
      if [ "$hnew" = "{}" ] && [ -z "$ag" ]; then rm -f "$GHK"; else mkdir -p "$(dirname "$GHK")"; printf '%s\n' "$hnew" > "$GHK"; fi
      changes+=("~/.gemini/config/hooks.json updated")
    fi
  fi
fi
# A real folder per skill holding a SKILL.md link (folder symlinks may not be followed).
for d in "$SYS"/skills/*/; do
  name="$(basename "$d")"; tdir="$GSK/$name"; tgt="$tdir/SKILL.md"; src="$VAULT/.kioku/skills/$name/SKILL.md"
  ours=""; [ -L "$tgt" ] && [[ "$(readlink "$tgt")" == */.kioku/skills/"$name"/SKILL.md ]] && ours=1
  if [ -z "$ag" ]; then
    if [ -n "$ours" ]; then rm "$tgt"; rmdir "$tdir" 2>/dev/null || true; changes+=("Antigravity skill removed: $name"); fi
    continue
  fi
  if [ -n "$ours" ] && [ "$(readlink "$tgt")" = "$src" ]; then continue
  elif [ -n "$ours" ]; then ln -sfn "$src" "$tgt"; changes+=("Antigravity skill moved to the new vault: $name")
  elif [ -e "$tdir" ] || [ -L "$tdir" ]; then warn "$tdir already exists and does not belong to Kioku; left untouched."
  else mkdir -p "$tdir"; ln -s "$src" "$tgt"; changes+=("Antigravity skill linked: $name")
  fi
done

# ---- 4. ~/.claude/settings.json (merged with jq) ----
[ -f "$SETTINGS" ] || echo '{}' > "$SETTINGS"
jq empty "$SETTINGS" || die "$SETTINGS is not valid JSON; left untouched."
[ -f "$SETTINGS.kioku.bak" ] || cp "$SETTINGS" "$SETTINGS.kioku.bak"
tmp="$(mktemp)"
jq --arg ss "$SS" --arg st "$ST" --arg up "$UP" --arg dir "$VAULT" --arg old "$OLD" --arg mode "$mode" '
  def drop(cmd): map(select(((.hooks // []) | map(.command) | index(cmd)) | not));
  def droplegacy(d): reduce ("session-start.sh", "stop-check.sh") as $h (.; drop("\(d)/.brain/hooks/\($h)"));
  def without(xs; ys): (xs // []) - ys;
  def rules(d): ["Read(/\(d)/**)", "Edit(/\(d)/**)"];
  ([$dir] + (if $old == "" then [] else [$old] end)) as $dirs
  | rules($dir) as $rules
  | ($dirs | map(rules(.)) | add) as $allrules
  | .hooks.SessionStart = ((.hooks.SessionStart // []) | drop($ss) | drop("\($old)/.kioku/hooks/session-start.sh") | droplegacy($dir) | droplegacy($old))
  | .hooks.Stop         = ((.hooks.Stop // []) | drop($st) | drop("\($old)/.kioku/hooks/stop-check.sh") | droplegacy($dir) | droplegacy($old))
  | .hooks.UserPromptSubmit = ((.hooks.UserPromptSubmit // []) | drop($up) | drop("\($old)/.kioku/hooks/prompt-pull.sh"))
  | .permissions.additionalDirectories = without(.permissions.additionalDirectories; $dirs)
  | .permissions.allow = without(.permissions.allow; $allrules)
  | .sandbox.filesystem.allowWrite = without(.sandbox.filesystem.allowWrite; $dirs)
  | if $mode == "--uninstall" then . else
      .hooks.SessionStart += [{hooks: [{type: "command", command: $ss, timeout: 10}]}]
    | .hooks.Stop         += [{hooks: [{type: "command", command: $st, timeout: 10}]}]
    | .hooks.UserPromptSubmit += [{hooks: [{type: "command", command: $up, timeout: 30}]}]
    | .permissions.additionalDirectories += [$dir]
    | .permissions.allow += $rules
    | .sandbox.filesystem.allowWrite += [$dir]
    end
  # Remove containers left empty
  | if (.hooks.SessionStart | length) == 0 then del(.hooks.SessionStart) else . end
  | if (.hooks.Stop | length) == 0 then del(.hooks.Stop) else . end
  | if (.hooks.UserPromptSubmit | length) == 0 then del(.hooks.UserPromptSubmit) else . end
  | if (.hooks | length) == 0 then del(.hooks) else . end
  | if (.permissions.additionalDirectories | length) == 0 then del(.permissions.additionalDirectories) else . end
  | if (.permissions.allow | length) == 0 then del(.permissions.allow) else . end
  | if (.permissions | length) == 0 then del(.permissions) else . end
  | if (.sandbox.filesystem.allowWrite | length) == 0 then del(.sandbox.filesystem.allowWrite) else . end
  | if (.sandbox.filesystem | length) == 0 then del(.sandbox.filesystem) else . end
  | if (.sandbox | length) == 0 then del(.sandbox) else . end
' "$SETTINGS" > "$tmp"
if ! cmp -s <(jq -S . "$SETTINGS") <(jq -S . "$tmp"); then
  cat "$tmp" > "$SETTINGS"; changes+=("~/.claude/settings.json updated (backup: settings.json.kioku.bak)")
fi
rm -f "$tmp"

# ---- 5. Record what was installed (kioku update and the session-start check read it) ----
rec="${XDG_CONFIG_HOME:-$HOME/.config}/kioku/installed"
if [ "$mode" = "--uninstall" ]; then
  rm -f "$rec"
else
  read -r ver fp < <("$SYS/bin/kioku-version" 2>/dev/null || echo "unknown unknown")
  mkdir -p "$(dirname "$rec")"
  printf 'version=%s\nfingerprint=%s\nvault=%s\nsource=%s\nflags=%s\n' "$ver" "$fp" "$VAULT" "$SYS" "$( { [ -n "$bin_link" ] || echo --no-bin-link; [ -n "$antigravity" ] || echo --no-antigravity; } | xargs)" > "$rec"
fi

# ---- Summary ----
if [ ${#changes[@]} -eq 0 ]; then echo "Kioku: no changes (already up to date). Vault: $VAULT"
else printf 'Kioku (vault: %s):\n' "$VAULT"; printf '  - %s\n' "${changes[@]}"; fi
if [ "$mode" = "--uninstall" ]; then
  echo "The vault and its .kioku link were kept. Delete them yourself if you no longer want them."
else
  [ -n "$ag" ] && echo "Antigravity: rules, skills and hooks set up; start a new conversation there."
  echo "Open a new Claude Code session; hooks and skills take effect there. Say \"get to know me\" to start."
fi
