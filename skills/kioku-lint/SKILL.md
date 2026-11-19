---
name: kioku-lint
description: Health check of the user's second brain (the Kioku vault) — broken links, orphan pages, index gaps, learnings missing from the wiki, contradictions, stale claims, missing concept pages, unclosed open threads. Use when the user says "check the brain", "clean up the brain", "brain lint" (Turkish: "beyni kontrol et", "beyni temizle"), or in the weekly maintenance task. NOT for linting code.
---

# kioku-lint

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (for example `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md` or `~/.gemini/GEMINI.md`).
Rules: `$VAULT/.kioku/SCHEMA.md` (Lint section); personal rules in `$VAULT/AGENTS.md`
CLI: `$VAULT/.kioku/bin/kioku`

## 1. Deterministic checks

Run `kioku lint-check` and keep its output. It exits with code 1 when it finds issues;
that is not an error.

Its **Learned, not in the wiki** section lists daily `learned:` lines without a `[[wiki/…]]` link. The fix for each: add the knowledge to its concept or entity page (create it if needed, with a `Topics:` line and a hub entry), then append the page link to the daily line.

## 2. Semantic checks

Read `index.md` and the pages. In a large wiki, start with the most recently updated and
most linked pages. Look for:

- **Contradictions:** do two pages say different things about the same topic, including across languages?
- **Stale claims:** the pages under *Due for review* (`kioku stale`): for each, the claims most likely to have changed (versions, services, prices, a person's job, a project's status). Also pages whose newer sources or daily entries say otherwise. In the report, ask the user about at most five pages ("Is this still true: …?"); on their answer set `reviewed:` to today, correct what changed (with a source), or mark it `(unverified)`. A page that will never change can get `review_every: never`.
- **Missing pages:** a concept or entity mentioned on many pages but without its own page.
- **Missing cross-links:** clearly related pages that don't link to each other.
- **Open threads:** `open:` entries in `daily/` that were done or became moot (propose changing them to `closed:`) or were forgotten; `projects/` notes with `status: active` that haven't been updated in a long time.
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
- Finally run `kioku lint-check` again to verify, then `kioku log lint "<summary>"`, and commit with `lint: <summary>`.
