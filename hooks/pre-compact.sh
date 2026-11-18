#!/usr/bin/env bash
# Kioku — PreCompact hook: its output becomes instructions for the compaction summary. It asks
# the summary to list what was not recorded to the vault yet; after the compaction the
# SessionStart hook (source "compact") asks Claude to record those items while the details are
# still known. Disable with KIOKU_STOP_CHECK=0 (same switch as the Stop reminder).
set -u
[ "${KIOKU_STOP_CHECK:-1}" = "0" ] && exit 0
VAULT="${KIOKU_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -L)}"
[ -d "$VAULT" ] || exit 0
cat >/dev/null 2>&1 || true
cat <<EOT
Kioku (the user's second brain, $VAULT): add a section to the summary titled "Kioku: not yet recorded". List, one line each, the decisions, finished results, durable learnings, user preferences and open threads from this conversation that have NOT been written to the vault yet (vault writes are file edits under $VAULT and kioku or git commit commands run there). Give each line the details needed to record it later: what, why, which project, file paths, dates. If everything was recorded or nothing is worth recording, write: "Kioku: not yet recorded: none".
EOT
