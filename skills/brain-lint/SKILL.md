---
name: brain-lint
description: Health check of the user's second brain (the Brain vault) — broken links, orphan pages, index gaps, contradictions, stale claims, missing concept pages, unclosed open threads. Use when the user says "check the brain", "clean up the brain", "brain lint" (Turkish: "beyni kontrol et", "beyni temizle"), or in the weekly maintenance task. NOT for linting code.
---

# brain-lint

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
Rules: `$BRAIN/.brain/SCHEMA.md` (Lint section); personal rules in `$BRAIN/CLAUDE.md`
CLI: `$BRAIN/.brain/bin/brain`

## 1. Deterministic checks

Run `brain lint-check` and keep its output. It exits with code 1 when it finds issues;
that is not an error.

## 2. Semantic checks

Read `index.md` and the pages. In a large wiki, start with the most recently updated and
most linked pages. Look for:

- **Contradictions:** do two pages say different things about the same topic, including across languages?
- **Stale claims:** an old `updated` date while a newer source or daily entry says otherwise.
- **Missing pages:** a concept or entity mentioned on many pages but without its own page.
- **Missing cross-links:** clearly related pages that don't link to each other.
- **Open threads:** `open:` entries in `daily/` that were closed or forgotten; `projects/` notes with `status: active` that haven't been updated in a long time.
- **Uncited claims:** wiki claims without a source.
- **Language drift:** pages whose content doesn't match their `lang:`, or that are missing `lang:`.
- **Categories:** wiki pages without a `Topics:` line or not listed in their hub; hubs that are nearly empty (merge them into a broader hub) or too big (split them, or add headings); two hubs covering the same thing; duplicate pages about the same subject (merge them).
- **Ideas:** ideas with `status: new` that have not been touched for a long time. Ask whether to keep, develop or drop each one.
- **Unprocessed inbox:** files waiting in `raw/inbox/`.

## 3. Report and approval

Show a short report in the user's language: bullet points per section, each with a
proposed fix. **Do not fix anything without approval.** The user may pick all, some or
none of the fixes.

## 4. Fix

- Apply only the approved fixes.
- Do not touch files under `raw/`.
- Prefer merging pages or adding a `> [!note] Outdated: …` callout over deleting.
- Add any new pages to `index.md`.
- Finally run `brain lint-check` again to verify, then `brain log lint "<summary>"`, and commit with `lint: <summary>`.
