---
name: kioku
description: The user's second brain (the Kioku vault). Use when a decision, finished work, durable learning, preference or open thread should be recorded; when the user says "remember this", "save to brain", "what did we do today/this week", "get to know me", "what's the status of the project" (or the same in Turkish, e.g. "bunu hatırla", "beyne kaydet", "bugün ne yaptık", "beni tanı"); or when the SessionStart context says Core.md is empty.
---

# kioku

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (for example `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md` or `~/.gemini/GEMINI.md`).
CLI: `$VAULT/.kioku/bin/kioku`

The rules are in `$VAULT/.kioku/SCHEMA.md`, plus personal rules in `$VAULT/AGENTS.md`. If you haven't read them in this session, read
them before writing.

The user does not write notes; recording is your job. Don't interrupt the conversation:
record quietly, then mention it in one line. Note content follows the conversation's
language; headings and entry types stay English.

## Which workflow?

| Request | Do |
| --- | --- |
| A decision, result, learning, preference or open thread | **Record** below |
| The user notes something that happened, or a todo/reminder | `kioku-journal` skill (journal/, not daily/) |
| The user wants to keep a tool, site, idea, command or something learned | `kioku-note` skill (wiki page + topic hub) |
| An appointment, class, meeting or any event with a time slot | `kioku-calendar` skill (calendar/) |
| A person's contact details | `kioku-contacts` skill (contacts/) |
| "What do we know about X?", anything needing past knowledge | `kioku-ask` skill |
| A link, PDF or article: "add it to the brain", "read and process this" | `kioku-ingest` skill |
| "Check the brain", "clean up", "brain lint" | `kioku-lint` skill |
| "What did we do today / this week?" | `kioku recap --days N`, then a short summary |
| "Get to know me", or Core is empty | **Onboarding** |

## Record

1. Open today's daily file with `kioku today`. One line per entry:
   `- HH:MM [project] decision|done|learned|preference|open: text ([[link]])`
   Infer the project from the session's folder or the topic.
2. If the work belongs to a project, update `projects/<slug>.md` (`status`, `next`, `updated`, History; `reviewed` too when you have just confirmed its Status section is current). If there is no note and the topic will recur, create one with `kioku new project "<Name>"`, set `path:` to the folder's absolute path, and add it to `index.md` › Projects.
3. Every `learned` entry also goes into the wiki: add it to the relevant `wiki/concepts/` or `wiki/entities/` page with `(source: [[daily/YYYY-MM-DD]])`; a new page gets a `Topics:` line and a line in its topic hub (see `kioku-note`). Then end the daily line with a link to the page, e.g. `([[wiki/concepts/x]])`. Without that link, `kioku lint-check` and the Stop hook report it as not done. Too narrow for the wiki? Then it is not `learned`: use `done`/`decision` or the project note.
4. Commit:
   ```
   git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "memory: <short title>"
   ```
5. Tell the user in one line: `Saved to Kioku: …`

If you learned something that belongs in `Core.md`, don't write it. Ask first: "Shall I add this to Core: …?"

## Onboarding

Only at a good moment: when the user asks, or offered after a task is done, never mid-task.

Onboarding is an open-ended interview. It has no fixed number of questions and keeps going
until the user ends it.

### Interview

1. **Open:** in one or two sentences, explain that you'll ask questions one at a time, that they can skip any question, and that they can end whenever they like by saying so ("bitir", "yeter", "that's enough", "stop" or anything similar).
2. **Ask one question per message:** short and natural, never a list of questions.
3. **Follow up on the answer:** the next question builds on what they just said: why, how, an example, what that looks like in practice. Go deeper while the answer opens up something worth knowing.
4. **Move on when the topic is clear:** once you understand a topic well enough to act on it, or the user's answers get short or they seem uninterested, switch to a new topic with a brief transition.
   Topics are a guide, not a script. Pick the order from the conversation and skip whatever is already known (from Core or other notes):
   - name and how to address them;
   - what they do (work, role, field);
   - what they are working on now and why it matters;
   - interests and things they want to learn;
   - tools, environment and how they set things up;
   - how they like to work with an assistant: tone, detail level, autonomy, when to ask;
   - things to avoid, pet peeves;
   - longer-term goals.
   When the topics run out, keep going with follow-ups or ask what else they think you should know.
5. **Never end the interview yourself.** Keep going until the user says to stop. Don't write to `Core.md` during the interview; keep what you learn in the conversation.

### After the user ends it

1. **Draft Core:** write a draft of `Core.md` in the user's language, using the headings already in the file. Use **only what the user said**: no guesses, no embellishment. Leave a heading empty if nothing was said about it.
2. **Show the draft** in a compact form and ask for corrections. This is the approval the schema requires for `Core.md`.
3. **Write it:** apply the corrections, write `Core.md`, delete the `<!-- kioku:empty … -->` line, set `lang:` and `updated`.
4. **Put the rest where it belongs:** facts that are not about the user themself go elsewhere. Active work goes into `projects/` notes (with `path:` when known). Tools and organizations go into `wiki/entities/` if they are worth a page. Add any new pages to `index.md`.
5. Run `kioku log note "Core: onboarding"` and commit with `core: onboarding`.
6. Ask the user to open a new session (ideally in another project) and ask "What does Core say?". Cross-session memory counts as verified only after that test.
