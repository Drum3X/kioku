---
name: kioku-calendar
description: The user's calendar in their second brain (calendar/ in the Kioku vault), one page per event, shown in the session context and in kioku agenda. Use when the user wants to add, move, cancel or list events and appointments — "takvime ekle", "cuma 14:00 diş randevusu", "her pazartesi 10'da ders", "toplantıyı perşembeye al", "yarınki etkinliği iptal et", "bu hafta neler var", "ajandam" (or "add to my calendar", "schedule", "what's on this week"). Todos and reminders without a fixed time slot go to kioku-journal.
argument-hint: "[event description]"
---

# kioku-calendar

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (`~/.claude/CLAUDE.md` in Claude Code, `~/.gemini/GEMINI.md` in Antigravity).
Rules: `$VAULT/.kioku/SCHEMA.md` ("Calendar, todos and contacts"); personal rules in `$VAULT/CLAUDE.md`
CLI: `$VAULT/.kioku/bin/kioku`

Events live in the vault, one page each: `calendar/<yyyy>/<YYYY-MM-DD>-<slug>.md`. The session
context and `kioku agenda` show the upcoming ones.

## Event or todo?

- **Event:** something with a time slot or a date to block: an appointment, class, meeting, trip, birthday party. It goes here.
- **Todo:** something to get done by a date ("submit homework by Friday"). It goes to `kioku-journal` (`- [ ] … (due: …)`).

## Add an event

1. Turn relative dates and times into absolute local ones (the system time zone). Examples: "cuma 14:00" → the coming Friday, "haftaya salı" → Tuesday next week. Say which date you used in your confirmation.
2. Create the page: `kioku new event "<Title>" YYYY-MM-DD`. It prints the path.
3. Fill the frontmatter:
   - `start`: `YYYY-MM-DDTHH:MM`, or `YYYY-MM-DD` for an all-day event (then set `all_day: true`).
   - `end`: an end time, or the last day of a multi-day all-day event (inclusive). If it is left empty: timed events last 1 hour (or set `duration: 90m`), all-day events last 1 day.
   - `location`, if given.
   - `rrule` for repeats, as an iCalendar RRULE: `FREQ=WEEKLY;BYDAY=MO`, `FREQ=DAILY;COUNT=5`, `FREQ=MONTHLY`, `FREQ=YEARLY`. Add `UNTIL=YYYYMMDD` for an end date ("until January" → `UNTIL=20270131`).
   - `alarm`: a reminder before the start, like `30m`, `2h` or `1d`. Only when the user asks, or for appointments where a reminder is clearly useful.
   - `status`: `confirmed` (default) or `tentative` (for "maybe", "probably").
   - `lang`: the conversation language.
4. Body: what the user said about it, plus links to related pages (`[[contacts/…]]`, `[[projects/…]]`).
5. Commit:
   ```
   git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "calendar: <title>"
   ```
6. Confirm in one line, in the user's language, for example: `Takvime eklendi: Diş randevusu, Cuma 21 Kasım 14:00–15:00`.

## Change, move or cancel

1. Find the page with `kioku agenda --days 30` or `kioku search <title>`.
2. Edit its frontmatter in place and update `updated`:
   - **Move:** change `start` and `end`.
   - **Cancel:** set `status: cancelled`. It leaves the agenda; the page stays in the vault as a record.
   - **Delete completely:** only if the user explicitly asks. Delete the file.
3. Commit with `calendar: <what changed>`, then confirm in one line.

## What's coming up?

"What's on this week?", "am I free on Thursday?": run `kioku agenda --days N` and answer briefly. It leaves out events that have already ended (add `--past` for "what was on today"); events marked `(now)` are under way. The session-start agenda is a snapshot from when the session began, so run it again rather than quoting that list later in a session. Include due todos from `kioku todo --soon` when they are relevant.
