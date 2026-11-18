---
name: kioku-ingest
description: Processes an external source (URL, article, paper, PDF, video transcript, thread, document) into the user's second brain (the Kioku vault) — saves it under raw/, writes a wiki/sources summary, updates related concept and entity pages. Use when the user says "add this to the brain", "ingest this", "process this article" (Turkish: "beyne at", "bunu işle"), or asks to process files waiting in raw/inbox.
argument-hint: "[URL | file path] (empty: list raw/inbox)"
---

# kioku-ingest

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (`~/.claude/CLAUDE.md` in Claude Code, `~/.gemini/GEMINI.md` in Antigravity).
Rules: `$VAULT/.kioku/SCHEMA.md` (Ingest and Languages sections); personal rules in `$VAULT/CLAUDE.md`
CLI: `$VAULT/.kioku/bin/kioku`

Process sources one at a time. If several were given, save all of them to `raw/` first,
then process each in turn. **Write summaries and new pages in the source's language.**

## 0. What to process

- **No argument:** list the files in `raw/inbox/` and ask where to start. Text captures (`type: capture`, from `kioku capture`) are routed as in the schema's "Quick captures" section; only links to read and files are ingested here.
- **URL:** fetch the content with your web-fetch tool (WebFetch in Claude Code). For long pages, read to the end (WebFetch: using offset). Ask for the actual text, not a summary.
- **File:** copy a PDF or image to `raw/assets/<yyyy>/`. Extract the text; for a PDF, read the pages with your file-reading tool (Read in Claude Code).

## 1. Save to raw/

File: `raw/<yyyy>/<slug>.md`. Once saved, **never modify it**. Use `kioku new source` once
to see the slug it produces, or follow the slug rule in the schema.

```yaml
---
title: <original title>
url: <source URL or empty>
author: <if known>
published: <if known, YYYY-MM-DD>
fetched: <today>
lang: <language code of the content>
type: article|paper|video|thread|doc|other
asset: <raw/assets/… if any>
---
```

Put the content below **as-is**: no summarizing, no commentary. Strip noise such as menus,
ads and footers. If the file came from inbox, move it to `raw/<yyyy>/`; that is the only
exception to the raw/ rule.

## 2. Discuss with the user

Show 3–5 key points, in the user's language. Point out links to and contradictions with the
existing wiki (read `index.md` first). Then ask what they care about and what to skip.
Skip this step if the user said "process without asking".

## 3. Source summary

1. Create the page with `kioku new source "<Title>"`.
2. Fill the frontmatter:
   - `lang` (the source's language);
   - `sources: [raw/<yyyy>/<slug>.md]`;
   - `tags`.
3. Write the body in the source's language:
   - **Summary:** 3–6 sentences.
   - **Key claims:** bullet points; each one must actually appear in the source.
   - **Links:** wikilinks to related concept and entity pages.
   - **Notes:** what the user said about this source.

## 4. Update the wiki

- Update the related `wiki/concepts/` and `wiki/entities/` pages, or create them with `kioku new`. Usually 3–15 pages are touched.
- A new page is written in the source's language. Additions to an existing page are written in that page's language (see its `lang:`).
- Cite every addition: `([[wiki/sources/<slug>]])`. Update `updated`.
- If something contradicts existing knowledge, don't overwrite it. Keep both views with their sources and add a `> [!warning] Contradiction` callout.
- If it relates to one of the user's projects, link it from the `projects/` note too.

## 5. Record and commit

1. For each new page, add one line under the right heading in `index.md`: `- [[path|Title]] — one sentence`.
2. Run `kioku log ingest "<Title>"`.
3. Commit:
   ```
   git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "ingest: <Title>"
   ```
4. Give the user a short list of the pages created and updated.
