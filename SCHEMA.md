# Brain — schema

The vault (`$BRAIN`, given in the Brain section of `~/.claude/CLAUDE.md`) is the user's
personal second brain: an Obsidian vault and a git repository. This file defines the rules of
the system. Follow it before writing anything to the vault.

This file is part of the shared system (`$BRAIN/.brain/` is a link to the second-brain
checkout). The vault's own `CLAUDE.md` imports it and holds the user's **personal rules**,
which take precedence over this file. When the user wants a rule changed for themselves, ask,
then write it under "Personal rules" in `$BRAIN/CLAUDE.md`. Don't edit this file.

**Core principle:** The user does not write notes by hand. You keep every record. The user
talks, shares links and approves; you write information to the right place, with its source,
briefly.

## Languages

- **System language is English:** this schema, skills, hooks, CLI, templates, section headings, frontmatter keys, log operations and daily entry types.
- **Note content is written in the language of its source:**
  - `raw/` stores the source exactly as it is.
  - A `wiki/sources/` summary is written in the source's language.
  - A new concept or entity page is written in the language of the source that created it. Later additions from other-language sources are written in the page's existing language, with a citation, so pages stay monolingual. Record the page language in `lang:`.
  - Conversation-derived notes (`daily/`, `journal/`, `projects/`, `Core.md`) are written in the language of the conversation.
- Reply to the user in the language they are using.

## Layers and ownership

| Path | What | Who writes |
| --- | --- | --- |
| `Core.md` | The user's identity, how to address them, preferences, working style | You, **only with the user's approval** |
| `daily/YYYY-MM-DD.md` | Your work log: decisions, finished work, learnings, open threads | You, without asking |
| `journal/YYYY-MM-DD.md` | The user's own daily notes: what they did, todos, ideas | You, whenever the user tells you something to note (`brain-journal`) |
| `projects/<slug>.md` | Active and past work: status, next step, links | You, without asking |
| `raw/` | Copies of external sources (articles, PDFs, transcripts) | You save them, **then never modify them** |
| `wiki/sources/` | One summary per source | You |
| `wiki/concepts/` | Concept pages | You |
| `wiki/entities/` | People, tools, organizations, products | You |
| `wiki/synthesis/` | Comparisons, valuable query answers | You, when the user wants |
| `index.md` | Wiki catalog, one line per page | You, on every new page |
| `log.md` | Append-only operation log | You, on every operation |
| `CLAUDE.md` | Imports this schema; the user's personal rules | You, only with the user's approval |
| `.brain/` | Link to the system checkout: schema, hooks, skills, CLI, templates | Never from the vault; system changes are made in the checkout |

## Tools

CLI: `$BRAIN/.brain/bin/brain`

- `brain today`: creates today's daily file if missing, prints its path.
- `brain journal`: creates today's journal file if missing, prints its path.
- `brain todo [--soon]`: open todos in `journal/` sorted by due date, with `file:line`; `--soon` shows only overdue ones and those due in the next 3 days.
- `brain new <source|concept|entity|synthesis|project> "<Title>"`: creates a page from its template, prints its path.
- `brain log <ingest|query|lint|note|setup> "<Title>"`: appends a line to `log.md`.
- `brain search [--raw] <term...>`: title matches first; `--raw` also searches `raw/`. Case-insensitive, Turkish İ/ı aware.
- `brain recap [--days N]`: daily notes and log entries from the last N days.
- `brain lint-check`: broken links, orphan pages, pages missing from the index, missing frontmatter.

Always create new pages with `brain new`, then edit them. Write files with Edit/Write.

## Memory: capturing from conversations

This is the most important workflow. The user does not take notes, so if you don't record
something, it is lost.

**Record without asking:**
- **decision:** what was chosen and why, and the rejected option if there was one.
- **done:** finished work and its result (commit, PR, file, link).
- **learned:** durable knowledge, the root cause of a problem, a method that worked.
- **preference:** the user adopting or rejecting an approach.
- **open:** unfinished work to come back to.

**Do not record:**
- Transient details.
- Things the code or git history already captures.
- Guesses.
- Conversation transcripts.
- Secrets (see Prohibited).

**How:**
1. Open the daily file with `brain today`. One line per entry:
   `- HH:MM [project] type: text ([[link]])`, where type is `decision|done|learned|preference|open`.
   Example: `- 14:20 [config] decision: stay on ripgrep, not qmd yet; fewer than 200 pages ([[wiki/concepts/llm-wiki]])`
2. If the work belongs to a project, update `projects/<slug>.md`: `status`, `next`, `updated`, and one line under History. If there is no project note and the work will recur, create one with `brain new project` and set `path:` to the project folder; the SessionStart hook will surface this note in sessions opened there.
3. If there is a durable **learned** item, add it to the relevant `wiki/concepts/` or `wiki/entities/` page with `(source: [[daily/YYYY-MM-DD]])`. If the page doesn't exist, create it and add it to `index.md`.
4. If you learned something about the user that belongs in `Core.md` (identity, preference, working style), don't write it; ask first: "Shall I add this to Core: …?"
5. After writing, tell the user in one line: `Saved to Brain: …`
6. Commit (see Git).

## Journal (the user's own notes)

When the user tells you something to note ("not al", "hatırlat", "yapılacaklara ekle", "bir
fikrim var", "bugün … yaptım"), follow the `brain-journal` skill. The note goes into
`journal/`, not `daily/`:
- `daily/` is your work log.
- `journal/` holds what the user wants to remember: things they did, todos with `due:` dates, ideas.

Journal pages stay out of the session-start context. Only due and overdue todos are shown
there. Write only what the user said; never fill the journal from your own work.

**When (memory):** when a piece of work is finished, when a decision is made, near the end of a
session. Not on every message. The Stop hook sometimes asks "is there anything to record?":
if so, record it; if not, say `Brain: nothing to record` in one line.

## Ingest (external source)

When the user shares a link or a file and says "add it to the brain" (in any language),
follow the `brain-ingest` skill. In short:

1. **Save:** to `raw/<yyyy>/<slug>.md` with frontmatter `title, url, author, fetched, lang, type: article|paper|video|thread|doc|other`. Put the content below as-is: no summarizing, no commentary. If there is a PDF or image, put the original under `raw/assets/` and link it.
2. **Discuss:** show 3–5 key points and ask what the user cares about. Skip this step if the user says "process without asking".
3. **Summarize:** `brain new source` → `wiki/sources/<slug>.md`, in the source's language. The raw path goes in `sources:`. Every claim must be grounded in the source.
4. **Connect:** update or create the related concept and entity pages (usually 3–15 pages). If something contradicts existing knowledge, don't overwrite it: keep both views with their sources and add a `> [!warning] Contradiction` callout.
5. **Record:** one `index.md` line per new page, `brain log ingest "<Title>"`, commit.

## Query

1. Read `index.md` first, then search with `brain search`, then read the relevant pages.
2. Answer with citations: `[[wiki/concepts/x]]`. If the brain doesn't have it, say so plainly; if you answer from general knowledge, mark it as such.
3. If the answer has lasting value (a comparison, a synthesis), offer to save it to `wiki/synthesis/`.
4. Run `brain log query "<question>"` only if the answer was saved.

## Lint (maintenance)

Follow the `brain-lint` skill:
1. Run `brain lint-check` first: broken links, orphan pages, pages missing from the index, missing frontmatter.
2. Then check semantically: contradictions, stale claims (old `updated` dates), concepts mentioned often but without their own page, missing cross-links, open threads that should be closed.
3. Show a report first and fix only after approval. Then `brain log lint`, commit.

## Writing rules

- **File names:** kebab-case slugs. Latin-script titles are transliterated to ASCII (`Çağdaş Öğrenme` → `cagdas-ogrenme.md`); non-Latin scripts are kept as-is (`машинное-обучение.md`, `機械学習.md`). The full title lives in `title:`. Always let `brain new` generate the slug.
- **Frontmatter** on every page:
  ```yaml
  ---
  title: …
  type: source|concept|entity|synthesis|project|daily|journal
  lang: tr|en|…      # language of the page content
  tags: []
  created: YYYY-MM-DD
  updated: YYYY-MM-DD
  sources: []        # raw/… or wiki/sources/… paths
  private: false
  ---
  ```
  Update `updated` whenever you change a page.
- **Links:** Obsidian wikilinks with the full path from the vault root: `[[wiki/concepts/llm-wiki|LLM Wiki]]`.
- **Citations:** every claim links to a source: `… ([[wiki/sources/x]])` or `(source: [[daily/…]])`.
- **Be brief.** Pages are working notes, not an encyclopedia. Prefer a summary and a link over a long quote.
- **`private: true`:** the page is kept out of the SessionStart context. Use it when the user says "keep this private".

## Prohibited

- Do not make things up. If unsure, don't write it, or mark it `(unverified)`.
- **Never** write secrets: passwords, API keys, tokens, card numbers, ID numbers. The repo is pushed to GitHub.
- Do not modify saved files under `raw/`.
- Do not change `Core.md` without the user's approval. Do not delete, move or rename existing notes.
- Do not touch `.brain/` unless the user asks.

## Git

- Make one commit at the end of every meaningful write:
  ```
  git -C "$BRAIN" add -A && git -C "$BRAIN" commit -m "<type>: <short title>"
  ```
  `<type>` is one of `memory`, `journal`, `ingest`, `query`, `lint`, `core`, `project`, `system`.
- Do not push. The user or a scheduled task pushes.

## Relation to Claude's built-in memory

Claude Code's auto memory is only for working preferences about how to help the user.
Knowledge, decisions, projects and the user's profile go into Brain. Never write the same
thing in both places.
