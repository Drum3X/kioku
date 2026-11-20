#!/usr/bin/env bash
# End-to-end memory test: every question of the given eval TSV files goes to a fresh, read-only
# Claude Code session (`claude -p`) in an empty folder. The answer must cite one of the right
# pages (or say the brain has nothing, for questions with no right page). This measures the
# whole loop: session context, skills, search, reading, answering. It uses the subscription's
# tokens: run it before a release, not on every change.
#
#   tests/e2e.sh [-j N] [-o DIR] FILE.tsv ...      (default: 3 at a time, results in a temp dir)
set -u
jobs=3 out=""
while [ $# -gt 0 ]; do
  case "$1" in
    -j) jobs="$2"; shift 2 ;;
    -o) out="$2"; shift 2 ;;
    *) break ;;
  esac
done
[ $# -gt 0 ] || { echo "usage: tests/e2e.sh [-j N] [-o DIR] FILE.tsv ..." >&2; exit 2; }
command -v claude >/dev/null || { echo "claude (Claude Code) is not on PATH" >&2; exit 2; }
out="${out:-$(mktemp -d -t kioku-e2e.XXXXXX)}"
work="$out/work"; mkdir -p "$work"

note='(This is a test of your memory. Answer from the second brain only, briefly. Do not write or change anything. End with one line "SOURCES: " followed by the vault paths of the pages your answer relies on, separated by spaces, or "SOURCES: none" if the brain has nothing on it.)'

n=0
: > "$out/cases.tsv"
for f in "$@"; do
  while IFS=$'\t' read -r q terms exp; do
    case "$q" in ''|'#'*) continue ;; esac
    n=$((n + 1))
    printf '%s\t%s\t%s\t%s\n' "$n" "$(basename "$f")" "$q" "$exp" >> "$out/cases.tsv"
  done < "$f"
done

run() {
  local i="$1" q="$2"
  ( cd "$work" && claude -p "$q

$note" --no-session-persistence --output-format text \
      --allowedTools "Read" "Grep" "Glob" "Bash(*kioku search*)" "Bash(*kioku todo*)" "Bash(*kioku agenda*)" \
        "Bash(*kioku recap*)" "Bash(*kioku context*)" "Bash(*kioku stale*)" "Bash(cat *)" "Bash(ls *)" \
      --disallowedTools "Edit" "Write" "NotebookEdit" \
      < /dev/null > "$out/$i.txt" 2> "$out/$i.err" )
}
while IFS=$'\t' read -r i q; do
  while [ "$(jobs -rp | wc -l)" -ge "$jobs" ]; do wait -n; done
  echo "[$i/$n] $q" >&2
  run "$i" "$q" &
done < <(cut -f1,3 "$out/cases.tsv")
wait

while IFS=$'\t' read -r i file q exp; do
  ans="$out/$i.txt"
  src="$(grep -a '^SOURCES:' "$ans" 2>/dev/null | tail -1 | sed 's/^SOURCES: *//')"
  if [ ! -s "$ans" ]; then
    r=ERROR
  elif [ -z "$exp" ]; then
    if [ "$src" = "none" ] || [ -z "$src" ]; then r=ok-none; else r=MISS-none; fi
  else
    r=MISS
    IFS='|' read -ra want <<< "$exp"
    for w in "${want[@]}"; do
      if grep -aqF "${w%.md}" "$ans"; then r=ok; break; fi
    done
  fi
  printf '%-9s %s:%s %s → %s\n' "$r" "$file" "$i" "$q" "${src:-(no SOURCES line)}"
done < "$out/cases.tsv" > "$out/report.txt"
cat "$out/report.txt"
c() { grep -c "^$1 " "$out/report.txt"; }
[ "$(c ERROR)" -gt 0 ] && echo "No answer (see the .err files): $(c ERROR)" | tee -a "$out/report.txt"
echo "Answered from a right page: $(c ok)/$(( $(c ok) + $(c MISS) )); said the brain has nothing (when it doesn't): $(c ok-none)/$(( $(c ok-none) + $(c MISS-none) ))" | tee -a "$out/report.txt"
echo "Answers: $out" >&2
