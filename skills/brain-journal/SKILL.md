---
name: brain-journal
description: The user's own daily notes in their second brain (journal/ in the Brain vault) — what they did, todos/reminders, ideas — taken instantly from chat in any session. Use when the user says "not al", "not et", "bunu günlüğe yaz", "hatırlat", "yapılacaklara ekle", "aklıma bir fikir geldi", "bugün … yaptım", "yapılacaklarım ne", "şunu yaptım, işaretle" (or "take a note", "remind me", "add a todo", "what are my todos"). Not for Claude's own work log (that is the brain skill / daily/).
argument-hint: "[note text]"
---

# brain-journal

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
CLI: `$BRAIN/.brain/bin/brain`

`journal/` holds **the user's own notes**. They tell you something and you write it down.
This is separate from `daily/`, which is your work log.

Journal pages are kept out of the session-start context; only due todos appear there.
So when the user asks about their past notes, search with `brain search`, `brain todo` or
`brain recap`.

## Taking a note (instant)

1. Run `brain journal` to get today's file. It is created if missing.
2. Decide which kind of note it is. One line per item, written in the user's words and in their language:

   | Kind | Section | Format |
   | --- | --- | --- |
   | Something they did or something that happened | `## Log` | `- HH:MM text` |
   | Todo / reminder | `## Todos` | `- [ ] text (due: YYYY-MM-DD)` |
   | Idea | `## Ideas` | `- HH:MM text ([[link]])` |

   - Turn relative dates ("yarın", "cuma", "haftaya") into an absolute `due:` date. Leave `due:` out if no date was given. If a date is ambiguous, pick the nearest future one and say which date you used.
   - If an idea or a log item relates to a project or a wiki page, add a wikilink (`[[projects/config]]`). Don't create new wiki pages for journal items unless the user asks.
   - One message can contain several items. Split them.
3. Commit:
   ```
   git -C "$BRAIN" add -A && git -C "$BRAIN" commit -qm "journal: <short title>"
   ```
4. Confirm in one short line, in the user's language, for example `Not alındı: … (yarın, 2026-11-18)`. Don't interrupt the current work any further.

Don't add your own interpretation. Write only what the user said, in a compact form.

## Todos

- **"What are my todos?"** Run `brain todo` and show them grouped: overdue, today, upcoming, no date.
- **"I did X" / "mark it done".** Find the matching line with `brain todo` (it prints `file:line`). Change `- [ ]` to `- [x]` in place and append ` (done: YYYY-MM-DD)`. Don't move the line. If more than one todo matches, ask which one.
- **Cancelled.** Change it to `- [-] text (cancelled: YYYY-MM-DD)`.
- **Postponed.** Edit `due:` on the same line.
- If the session-start context lists due or overdue todos, mention them once, briefly, at a natural moment (not in the middle of a task). Don't nag.

## Looking back

- "What did I do this week?": run `brain recap --days 7` and summarize the `journal` entries (and the `daily` entries if relevant).
- "Did I have an idea about X?": run `brain search X` and look at results under `journal/`.
