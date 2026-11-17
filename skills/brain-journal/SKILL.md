---
name: brain-journal
description: The user's timeline in their second brain (journal/ in the Brain vault) — what they did today, what happened, todos and reminders with due dates — taken instantly from chat in any session. Use when the user says "not al: bugün …", "bugün … yaptım", "hatırlat", "yapılacaklara ekle", "yapılacaklarım ne", "şunu yaptım, işaretle" (or "today I …", "remind me", "add a todo", "what are my todos"). Durable things (tools, sites, ideas, things learned) go to brain-note; Claude's own work log is the brain skill / daily/.
argument-hint: "[note text]"
---

# brain-journal

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
CLI: `$BRAIN/.brain/bin/brain`

`journal/` is **the user's timeline**: what happened and what they have to do. It is separate
from `daily/` (your work log) and from the wiki, where durable notes are compiled
(`brain-note`).

Journal pages are kept out of the session-start context; only due todos appear there.
So when the user asks about their past notes, search with `brain search`, `brain todo` or
`brain recap`.

## Taking a note (instant)

1. **Split and classify.** One message can hold several items. Decide for each one:
   - something they did or something that happened → `## Log` (below);
   - a todo or reminder → `## Todos` (below);
   - a tool, site, app, person, game…, an idea, a command, a how-to or something learned → **`brain-note`**. It goes to the wiki and leaves a line in `## Saved`;
   - an article to read and digest → suggest `brain-ingest`.
2. Run `brain journal` to get today's file. It is created if missing.
3. Write one line per item, in the user's words and in their language:

   | Kind | Section | Format |
   | --- | --- | --- |
   | Something they did or something that happened | `## Log` | `- HH:MM text` |
   | Todo / reminder | `## Todos` | `- [ ] text (due: YYYY-MM-DD)` |
   | Pointer to a durable note (written by `brain-note`) | `## Saved` | `- HH:MM saved → [[path\|Title]]` |

   - Turn relative dates ("yarın", "cuma", "haftaya") into an absolute `due:` date. Leave `due:` out if no date was given. If a date is ambiguous, pick the nearest future one and say which date you used.
   - If the todo has a time ("cuma 17:00'ye kadar"), add `(at: HH:MM)` after `due:`. A synced todo with a time rings a reminder on the user's devices at that time.
   - If the user wants an earlier reminder ("bir saat önce hatırlat", "remind me the day before"), add `(remind: 1h)`. Other forms: `30m`, `1d`, or `0` for exactly at the due time. Without `at:`, the reminder counts back from 09:00 on the due date. Example: `- [ ] ödev teslimi (due: 2026-11-20) (at: 17:00) (remind: 2h)`.
   - If a log item or todo relates to a project or a wiki page, add a wikilink (`[[projects/config]]`).
   - Don't type ids yourself. The pre-commit hook adds a block id like ` ^t-3f9a1c` to each todo, and the id keeps the todo linked to its copy on the server. Never change or remove an existing `^t-…` id.
   - Something with a time slot (an appointment, a class, a meeting) is an event, not a todo; it goes to `brain-calendar`.
4. Commit:
   ```
   git -C "$BRAIN" add -A && git -C "$BRAIN" commit -qm "journal: <short title>"
   ```
5. Confirm in one short line, in the user's language, for example `Not alındı: … (yarın, 2026-11-18)`. Don't interrupt the current work any further.

Don't add your own interpretation. Write only what the user said, in a compact form.

## Todos

- **"What are my todos?"** Run `brain todo` and show them grouped: overdue, today, upcoming, no date.
- **"I did X" / "mark it done".** Find the matching line with `brain todo` (it prints `file:line`). Change `- [ ]` to `- [x]` in place and append ` (done: YYYY-MM-DD)`. Don't move the line. If more than one todo matches, ask which one.
- **Cancelled.** Change it to `- [-] text (cancelled: YYYY-MM-DD)`.
- **Postponed.** Edit `due:` on the same line.
- If the session-start context lists due or overdue todos, mention them once, briefly, at a natural moment (not in the middle of a task). Don't nag.

## Looking back

- "What did I do this week?": run `brain recap --days 7` and summarize the `journal` entries (and the `daily` entries if relevant).
- "What did I save this week?": the `## Saved` lines in `brain recap`.
- "Did I have an idea about X?": look in `wiki/ideas/` and the topic hubs (see `brain-note`), then `brain search X`.
