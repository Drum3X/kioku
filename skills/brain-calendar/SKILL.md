---
name: brain-calendar
description: The user's calendar in their second brain (calendar/ in the Brain vault), synced one-way to their CalDAV server so it shows in Thunderbird and other clients. Use when the user wants to add, move, cancel or list events and appointments — "takvime ekle", "cuma 14:00 diş randevusu", "her pazartesi 10'da ders", "toplantıyı perşembeye al", "yarınki etkinliği iptal et", "bu hafta neler var", "ajandam" (or "add to my calendar", "schedule", "what's on this week"). Todos and reminders without a fixed time slot go to brain-journal.
argument-hint: "[event description]"
---

# brain-calendar

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
Rules: `$BRAIN/.brain/SCHEMA.md` ("Calendar, todos and contacts"); personal rules in `$BRAIN/CLAUDE.md`
CLI: `$BRAIN/.brain/bin/brain`

Events live in the vault, one page each: `calendar/<yyyy>/<YYYY-MM-DD>-<slug>.md`. The vault
is the source. After each commit, a git hook pushes them to the user's CalDAV calendar, and
Thunderbird shows them from there. An edit made in Thunderbird to a Brain event is overwritten
on the next sync, so changes always go through you.

## Event or todo?

- **Event:** something with a time slot or a date to block: an appointment, class, meeting, trip, birthday party. It goes here.
- **Todo:** something to get done by a date ("submit homework by Friday"). It goes to `brain-journal` (`- [ ] … (due: …)`), and it also syncs as a task.

## Add an event

1. Turn relative dates and times into absolute local ones (the system time zone). Examples: "cuma 14:00" → the coming Friday, "haftaya salı" → Tuesday next week. Say which date you used in your confirmation.
2. Create the page: `brain new event "<Title>" YYYY-MM-DD`. It prints the path.
3. Fill the frontmatter:
   - `start`: `YYYY-MM-DDTHH:MM`, or `YYYY-MM-DD` for an all-day event (then set `all_day: true`).
   - `end`: an end time, or the last day of a multi-day all-day event (inclusive). If it is left empty: timed events last 1 hour (or set `duration: 90m`), all-day events last 1 day.
   - `location`, if given.
   - `rrule` for repeats, as an iCalendar RRULE: `FREQ=WEEKLY;BYDAY=MO`, `FREQ=DAILY;COUNT=5`, `FREQ=MONTHLY`, `FREQ=YEARLY`. Add `UNTIL=YYYYMMDD` for an end date ("until January" → `UNTIL=20270131`).
   - `alarm`: a reminder before the start, like `30m`, `2h` or `1d`. Only when the user asks, or for appointments where a reminder is clearly useful.
   - `status`: `confirmed` (default) or `tentative` (for "maybe", "probably").
   - `lang`: the conversation language.
   - Leave `uid` empty; the pre-commit hook fills it in.
4. Body: what the user said about it, plus links to related pages (`[[contacts/…]]`, `[[projects/…]]`).
5. Commit:
   ```
   git -C "$BRAIN" add -A && git -C "$BRAIN" commit -qm "calendar: <title>"
   ```
   The commit triggers the sync.
6. Confirm in one line, in the user's language, for example: `Takvime eklendi: Diş randevusu, Cuma 21 Kasım 14:00–15:00`.

## Change, move or cancel

1. Find the page with `brain agenda --days 30` or `brain search <title>`.
2. Edit its frontmatter in place and update `updated`:
   - **Move:** change `start` and `end`.
   - **Cancel:** set `status: cancelled`. The event is removed from the server; the page stays in the vault as a record.
   - **Delete completely:** only if the user explicitly asks. Delete the file; the sync then removes the event from the server.
3. Commit with `calendar: <what changed>`, then confirm in one line.

## What's coming up?

"What's on this week?", "am I free on Thursday?": run `brain agenda --days N` and answer briefly. Include due todos from `brain todo --soon` when they are relevant.

## If the sync fails

The sync runs in the background and writes to `~/.local/state/second-brain/sync.log`. If the
user says an event is missing in Thunderbird:
1. Run `brain sync --check`. It shows the connection, the calendar it found and the item counts.
2. Run `brain sync`.

Report errors plainly. Never ask for, read or handle the password. It lives outside the vault, in `~/.config/second-brain/dav.pass` (or a `password_cmd`), and only the user edits it.
