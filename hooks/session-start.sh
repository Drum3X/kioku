#!/usr/bin/env bash
# Brain — SessionStart hook.
# Adds Core.md, recent daily notes, active projects, the note for the current folder and a
# wiki summary to Claude's context at session start. Exits silently on any error so it
# never breaks a session.
set -u
export PATH="/etc/profiles/per-user/${USER:-$(id -un)}/bin:/run/current-system/sw/bin:$HOME/.nix-profile/bin:$PATH"

# Vault = two levels above this script as reached through the vault's .brain link (logical
# path, symlinks not resolved). BRAIN_DIR overrides.
BRAIN="${BRAIN_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
MAX_BYTES="${BRAIN_CONTEXT_MAX:-12000}"
input="$(cat 2>/dev/null || true)"
[ -d "$BRAIN" ] || exit 0
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

add "# Brain — session context"
add "The user's second brain: $BRAIN (rules: $BRAIN/.brain/SCHEMA.md and personal rules in $BRAIN/CLAUDE.md — read before writing to the vault)."
add "The user does not write notes: you record decisions, finished work and durable learnings (brain skill). Note content follows the source/conversation language; system text is English."
add ""

# 1. Core
if [ -f "$BRAIN/Core.md" ]; then
  if grep -q 'brain:empty' "$BRAIN/Core.md"; then
    add "## Core (empty)"
    add "Core.md has not been filled in. At a good moment (without interrupting work), offer to get to know the user: Onboarding in the brain skill."
  else
    add "## Core"
    add "$(body "$BRAIN/Core.md" | compact | cap 4000)"
  fi
  add ""
fi

# 2. Note for this folder: a project whose path: contains cwd, or a project/entity named like cwd
match=""
base="$(basename "$cwd" | tr '[:upper:]' '[:lower:]')"
for f in "$BRAIN"/projects/*.md; do
  [ -f "$f" ] || continue
  is_private "$f" && continue
  p="$(fm "$f" path)"; p="${p/#\~/$HOME}"
  if { [ -n "$p" ] && case "$cwd/" in "${p%/}/"*) true;; *) false;; esac; } || [ "$(basename "$f" .md)" = "$base" ]; then
    match="$f"; break
  fi
done
if [ -z "$match" ] && [ -f "$BRAIN/wiki/entities/$base.md" ] && ! is_private "$BRAIN/wiki/entities/$base.md"; then
  match="$BRAIN/wiki/entities/$base.md"
fi
if [ -n "$match" ]; then
  add "## Note for this folder: ${match#"$BRAIN"/}"
  add "$(body "$match" | compact | cap 3000)"
  add ""
fi

# 3. Active projects
act=""
for f in "$BRAIN"/projects/*.md; do
  [ -f "$f" ] || continue
  is_private "$f" && continue
  [ "$(fm "$f" status)" = "active" ] || continue
  rel="${f#"$BRAIN"/}"
  act+="- [[${rel%.md}]] $(fm "$f" title) — next: $(fm "$f" next)"$'\n'
done
if [ -n "$act" ]; then add "## Active projects"; add "$(printf '%s' "$act" | head -n 15)"; add ""; fi

# 3b. Due todos from the user's journal (the rest of journal/ stays out of the context)
todos="$("$BRAIN/.brain/bin/brain" todo --soon 2>/dev/null | grep -v '^(no open todos)$' | head -n 12 || true)"
if [ -n "$todos" ]; then
  add "## Due todos (journal)"
  add "Mention overdue or due-today items to the user once, briefly, at a natural moment."
  add "$todos"
  add ""
fi

# 4. Last 2 daily notes
days="$(ls -1 "$BRAIN"/daily/[0-9]*.md 2>/dev/null | sort | tail -n 2)"
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
cnt() { find "$BRAIN/wiki/$1" -name '*.md' 2>/dev/null | wc -l | tr -d ' '; }
add "## Wiki"
add "Topics $(cnt topics) · Concepts $(cnt concepts) · Entities $(cnt entities) · Ideas $(cnt ideas) · Sources $(cnt sources) · Syntheses $(cnt synthesis) · $(find "$BRAIN/raw/inbox" -type f ! -name .gitkeep 2>/dev/null | wc -l | tr -d ' ') unprocessed in inbox. For questions needing past knowledge, read index.md and the topic hubs first or use '$BRAIN/.brain/bin/brain search'."
if [ -f "$BRAIN/log.md" ]; then
  add "Recent operations:"
  add "$(grep '^## \[' "$BRAIN/log.md" | tail -n 5 | sed 's/^## /- /')"
fi

ctx="$(printf '%s' "$out" | cap "$MAX_BYTES" | iconv -c -f UTF-8 -t UTF-8 2>/dev/null || printf '%s' "$out" | cap "$MAX_BYTES")"
jq -n --arg c "$ctx" '{hookSpecificOutput:{hookEventName:"SessionStart",additionalContext:$c}}'
