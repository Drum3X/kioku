---
name: brain-note
description: Saves durable notes the user tells you into their second brain's wiki (the Brain vault) as interlinked, categorized pages — useful tools, sites, apps, libraries, people, games, anime, commands, how-tos, facts they learned, and ideas. Use when the user says "not al", "kaydet", "bunu beyne at", "işime yarar", "faydalı bir site/araç", "fikir", "aklıma bir şey geldi", "şunu öğrendim" (or "save this", "note this tool", "I have an idea", "TIL") about something worth keeping beyond today. Time-bound notes and todos go to brain-journal; articles to read go to brain-ingest.
argument-hint: "[what to save]"
---

# brain-note

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
Rules: `$BRAIN/.brain/SCHEMA.md` ("The user's notes" and "Topics" sections); personal rules in `$BRAIN/CLAUDE.md`
CLI: `$BRAIN/.brain/bin/brain`

Notes are compiled into the wiki: one page per subject, which grows over time, linked to
related pages and listed in topic hubs. The user should be able to browse everything in
Obsidian through `index.md` → topic hub → page.

## 1. Classify

Split the message into items. For each item:

| Item | Page |
| --- | --- |
| A named thing: tool, site, app, library, service, person, org, game, anime… | `wiki/entities/` — `brain new tool` for tools/sites/apps, `brain new entity` otherwise; set `kind:` |
| A method, command, how-to, fact, something learned | `wiki/concepts/` — `brain new concept` |
| An idea | `wiki/ideas/` — `brain new idea` |
| Something that happened, or a todo | not here: `brain-journal` |
| An article, paper or video to digest | not here: suggest `brain-ingest` |

If an item is ambiguous between a quick save and a full ingest (for example a link to a long
article), ask in one short question.

## 2. Find the subject page

Read `$BRAIN/index.md` › Topics and run `brain search <name>`. Also search for the URL if
there is one.

- **The subject already has a page:** add the note there. Put it in `## Notes` for an entity, the matching section for a concept, or `## Idea` / `## Next step` for an idea. Update `updated`. This is the normal case for "one more thing about Kate / Nix / X".
- **A small fact about an existing subject** goes into that subject's page. It does not get a page of its own. For example, a Kate shortcut goes into the Kate page.
- **No page:** create one with `brain new …`.

## 3. Write the page

- **Frontmatter:**
  - `lang`: the conversation language;
  - `tags`: 1–4 fine-grained facets;
  - for entities: `kind`, and `url` if known;
  - for ideas: `status: new`.
- **URL given:** fetch it once with WebFetch to learn what the thing is (name, purpose, platform, license or price if obvious). Write that in 1–2 sentences under `## What it is`, with the link. Don't save the page to `raw/`; this is a pointer, not a source.
- **`## Why it's useful` / `## Why`:** what the user said, in their words. If they gave no reason, write the context it came up in. Never invent one.
- **Links:** add wikilinks to related existing pages and projects. Where it fits, add a back-link line on those pages too (for example under `## Related`, or a project's Decisions/Status).

## 4. Categorize

1. Choose 1–2 topic hubs from `index.md` › Topics. Prefer existing ones. Use a heading inside a broader hub before creating a new hub.
2. If nothing fits, create a hub with `brain new topic "<Name>"` (in the conversation language). Write a one-sentence `## Overview` and add it to `index.md` › Topics as `- [[wiki/topics/x|Name]] — one sentence`.
3. Put the topic line right under the page's H1: `Topics: [[wiki/topics/x|X]] · [[wiki/topics/y|Y]]`.
4. List the page in each hub, under the matching group heading (`## Tools`, `## Sites`, `## Concepts`, `## Ideas`, `## People`, `## Projects`, …). Create the heading if it is missing. The line format is `- [[path|Title]] — one sentence`.

## 5. Leave a trace in the journal

Run `brain journal`. Add one line under `## Saved`, for example
`- HH:MM saved → [[wiki/entities/ghostty|Ghostty]]`. For an update to an existing page, use
`updated → …`.

## 6. Commit and confirm

1. Commit:
   ```
   git -C "$BRAIN" add -A && git -C "$BRAIN" commit -qm "note: <short title>"
   ```
2. Confirm in one line, in the user's language, naming the page and its topic, for example:
   `Kaydedildi: Ghostty → Linux › Tools`.
   If you created a hub, add `New topic: …` (in the user's language).
3. Don't interrupt the current work further.

## Looking things up

"Which tool did I save for X?", "my game ideas", "what do I know about Nix?": start from the
hub (`wiki/topics/…`), then the pages, then `brain search`. Answer with links, as in
`brain-ask`.
