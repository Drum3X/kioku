#!/usr/bin/env bash
# Kioku — SessionStart hook.
# Adds Core.md, recent daily notes, active projects, the note for the current folder and a
# wiki summary to Claude's context at session start. Exits silently on any error so it
# never breaks a session.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

# Vault = two levels above this script as reached through the vault's .kioku link (logical
# path, symlinks not resolved). KIOKU_DIR overrides.
VAULT="${KIOKU_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
MAX_BYTES="${KIOKU_CONTEXT_MAX:-12000}"
input="$(cat 2>/dev/null || true)"
[ -d "$VAULT" ] || exit 0
command -v jq >/dev/null || exit 0

cwd="$(jq -r '.cwd // empty' <<<"$input" 2>/dev/null || true)"
[ -n "$cwd" ] || cwd="$PWD"

# One field from the YAML frontmatter (first match).
fm() { awk -v k="$2" 'NR==1&&$0!="---"{exit} NR>1&&$0=="---"{exit} NR>1{ if (index($0,k":")==1){ sub("^"k":[ \t]*",""); print; exit } }' "$1" 2>/dev/null; }
is_private() { [ "$(fm "$1" private)" = "true" ]; }
# Body without frontmatter and without single-line HTML comments.
body() { awk 'NR==1&&$0=="---"{f=1;next} f&&$0=="---"{f=0;next} !f' "$1" | sed '/^[[:space:]]*<!--.*-->[[:space:]]*$/d'; }
# Drop blank lines and headings with nothing under them.
compact() { sed '/^[[:space:]]*$/d' | awk '/^#/{h=$0; next} {if(h!=""){print h; h=""} print}'; }
cap() { head -c "$1"; }

out=""
add() { out+="$1"$'\n'; }

# Several copies of the vault (kioku remote on): fetch the other machines' notes first.
st="${XDG_STATE_HOME:-$HOME/.local/state}/kioku"
if [ "$(git -C "$VAULT" config --bool kioku.autosync 2>/dev/null)" = true ]; then
  "$VAULT/.kioku/bin/kioku" remote pull >/dev/null 2>&1 || true
fi

add "# Kioku — session context"
add "The user's second brain: $VAULT (rules: $VAULT/.kioku/SCHEMA.md and personal rules in $VAULT/CLAUDE.md — read before writing to the vault)."
add "The user does not write notes: you record decisions, finished work and durable learnings (kioku skill). Note content follows the source/conversation language; system text is English."
# Kioku updated (git pull) without its install step: skills or settings may be stale.
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -L)"
if [ -x "$here/bin/kioku-version" ]; then
  fp_now="$("$here/bin/kioku-version" 2>/dev/null | cut -d' ' -f2)"
  fp_inst="$(sed -n 's/^fingerprint=//p' "${XDG_CONFIG_HOME:-$HOME/.config}/kioku/installed" 2>/dev/null)"
  if [ -n "$fp_now" ] && [ "$fp_now" != "$fp_inst" ]; then
    add "Kioku was updated but its install step has not run since, so skills or settings may be out of date. Tell the user once and offer to run \`$here/bin/kioku update\` (with the Nix module: rebuild)."
  fi
fi
if [ -f "$st/git-sync.conflict" ]; then
  add "Vault sync problem: $(cat "$st/git-sync.conflict" 2>/dev/null). Tell the user once; offer to resolve it (git -C $VAULT pull --rebase, fix the conflict keeping both sides, then kioku remote now)."
fi
add ""

# 1. Core
if [ -f "$VAULT/Core.md" ]; then
  if grep -qE '(kioku|brain):empty' "$VAULT/Core.md"; then
    add "## Core (empty)"
    add "Core.md has not been filled in. At a good moment (without interrupting work), offer to get to know the user: Onboarding in the kioku skill."
  else
    add "## Core"
    add "$(body "$VAULT/Core.md" | compact | cap 4000)"
  fi
  add ""
fi

# 2. Note for this folder: a project whose path: contains cwd, or a project/entity named like cwd
match=""
base="$(basename "$cwd" | tr '[:upper:]' '[:lower:]')"
for f in "$VAULT"/projects/*.md; do
  [ -f "$f" ] || continue
  is_private "$f" && continue
  p="$(fm "$f" path)"; p="${p/#\~/$HOME}"
  if { [ -n "$p" ] && case "$cwd/" in "${p%/}/"*) true;; *) false;; esac; } || [ "$(basename "$f" .md)" = "$base" ]; then
    match="$f"; break
  fi
done
if [ -z "$match" ] && [ -f "$VAULT/wiki/entities/$base.md" ] && ! is_private "$VAULT/wiki/entities/$base.md"; then
  match="$VAULT/wiki/entities/$base.md"
fi
if [ -n "$match" ]; then
  add "## Note for this folder: ${match#"$VAULT"/}"
  add "$(body "$match" | compact | cap 3000)"
  add ""
fi

# 3. Active projects
act=""
for f in "$VAULT"/projects/*.md; do
  [ -f "$f" ] || continue
  is_private "$f" && continue
  [ "$(fm "$f" status)" = "active" ] || continue
  rel="${f#"$VAULT"/}"
  act+="- [[${rel%.md}]] $(fm "$f" title) — next: $(fm "$f" next)"$'\n'
done
if [ -n "$act" ]; then add "## Active projects"; add "$(printf '%s' "$act" | head -n 15)"; add ""; fi

# 3b. Due todos from the user's journal (the rest of journal/ stays out of the context)
todos="$("$VAULT/.kioku/bin/kioku" todo --soon 2>/dev/null | grep -v '^(no open todos)$' | head -n 12 || true)"
if [ -n "$todos" ]; then
  add "## Due todos (journal)"
  add "Mention overdue or due-today items to the user once, briefly, at a natural moment."
  add "$todos"
  add ""
fi

# 3c. Upcoming events (calendar/), next 3 days
if command -v node >/dev/null && [ -d "$VAULT/calendar" ]; then
  agenda="$(KIOKU_DIR="$VAULT" node "$VAULT/.kioku/bin/kioku-sync.mjs" agenda --days 3 2>/dev/null | grep -v '^(no events' | head -n 12 || true)"
  if [ -n "$agenda" ]; then
    add "## Upcoming events (next 3 days)"
    add "$agenda"
    add ""
  fi
fi

# 4. Last 2 daily notes
days="$(ls -1 "$VAULT"/daily/[0-9]*.md 2>/dev/null | sort | tail -n 2)"
if [ -n "$days" ]; then
  add "## Recent daily notes"
  while IFS= read -r f; do
    is_private "$f" && continue
    add "### $(basename "$f" .md)"
    add "$(body "$f" | grep -v '^# ' | sed '/^[[:space:]]*$/d' | tail -n 25 | cap 2500)"
  done <<<"$days"
  add ""
fi

# 5. Wiki summary and recent operations
cnt() { find "$VAULT/wiki/$1" -name '*.md' 2>/dev/null | wc -l | tr -d ' '; }
add "## Wiki"
add "Topics $(cnt topics) · Concepts $(cnt concepts) · Entities $(cnt entities) · Ideas $(cnt ideas) · Sources $(cnt sources) · Syntheses $(cnt synthesis) · $(find "$VAULT/raw/inbox" -type f ! -name .gitkeep 2>/dev/null | wc -l | tr -d ' ') unprocessed in inbox. For questions needing past knowledge, read index.md and the topic hubs first or use '$VAULT/.kioku/bin/kioku search'."
if [ -f "$VAULT/log.md" ]; then
  add "Recent operations:"
  add "$(grep '^## \[' "$VAULT/log.md" | tail -n 5 | sed 's/^## /- /')"
fi

ctx="$(printf '%s' "$out" | cap "$MAX_BYTES" | iconv -c -f UTF-8 -t UTF-8 2>/dev/null || printf '%s' "$out" | cap "$MAX_BYTES")"
jq -n --arg c "$ctx" '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$c}}'
