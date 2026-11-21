#!/usr/bin/env bash
# Smoke test of a built kioku (CI on Linux and on Windows in Git Bash): a fixture vault in a
# temporary home, the install step with Claude Code present, then health, search, context, the
# session-start hook and a commit through the post-commit hook. Needs target/release/kioku[.exe].
set -eu
SYS="$(cd "$(dirname "$0")/.." && pwd)"
EXE=kioku; [ -f "$SYS/target/release/kioku.exe" ] && EXE=kioku.exe
mkdir -p "$SYS/bin"; [ "$SYS/bin/$EXE" -ef "$SYS/target/release/$EXE" ] || cp -f "$SYS/target/release/$EXE" "$SYS/bin/$EXE"
win() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else echo "$1"; fi; }
fail() { echo "SMOKE FAIL: $*" >&2; exit 1; }

T="$(mktemp -d)"
export HOME="$T/home" XDG_CONFIG_HOME="" XDG_STATE_HOME="" XDG_CACHE_HOME=""
export USERPROFILE="$(win "$HOME")" APPDATA="$(win "$T/appdata")" LOCALAPPDATA="$(win "$T/local")"
mkdir -p "$HOME/.claude" "$T/appdata" "$T/local"
V="$T/vault"
bash "$SYS/tests/make-fixture.sh" "$V" >/dev/null
rm -rf "$V/.kioku"   # the fixture links it; on Windows ln -s copies, so let install link it
git -C "$V" init -q
git -C "$V" config user.email smoke@example.invalid
git -C "$V" config user.name smoke
git -C "$V" add -A && git -C "$V" commit -qm fixture

"$SYS/bin/$EXE" install --vault "$(win "$V")" --no-bin-link || fail "install"
K="$V/.kioku/bin/$EXE"
[ -f "$K" ] || fail ".kioku link: $K missing"
"$K" version
"$K" health | tee "$T/health.txt"
grep -q "Claude Code" "$T/health.txt" || fail "health has no Claude Code section"
grep -q "kioku" "$HOME/.claude/settings.json" || fail "no hooks in ~/.claude/settings.json"
grep -q "hook claude session-start" "$HOME/.claude/settings.json" || fail "session-start hook not wired"
"$K" lint-check > /dev/null || true
"$K" search alpha | head -3
"$K" context --agent smoke | grep -q "Kioku — session context" || fail "context"
printf '{"session_id":"smoke","cwd":"%s"}' "$(win "$V")" | "$K" hook claude session-start | grep -q "session context" || fail "session-start hook"

# A commit fires the post-commit hook, which refreshes Home.md in the background (kioku bg).
rm -f "$V/Home.md"
echo "- 12:00 [alpha] done: smoke commit" >> "$V/daily/$(date +%F).md"
git -C "$V" add -A && git -C "$V" commit -qm smoke
for _ in $(seq 1 20); do [ -f "$V/Home.md" ] && break; sleep 0.5; done
[ -f "$V/Home.md" ] || fail "post-commit hook did not refresh Home.md"

# A secret is reported by the write check and stopped by the pre-commit hook.
printf -- '---\ntitle: S\n---\n# S\nghp_0123456789abcdefABCDEF0123456789abcd\n' > "$V/wiki/concepts/s.md"
"$K" check-file "$(win "$V/wiki/concepts/s.md")" | grep -q "looks like a secret" || fail "check-file missed a secret"
git -C "$V" add -A
if git -C "$V" commit -qm secret 2>/dev/null; then fail "pre-commit hook let a secret in"; fi
git -C "$V" reset -q HEAD -- . && rm -f "$V/wiki/concepts/s.md"

"$K" install --vault "$(win "$V")" --uninstall --no-bin-link > /dev/null || fail "uninstall"
grep -q "hook claude" "$HOME/.claude/settings.json" 2>/dev/null && fail "hooks left after uninstall"
echo "smoke OK"
rm -rf "$T"
