---
name: kioku-dream
description: The weekly dream pass over the user's second brain (the Kioku vault) — consolidates the last week's work log on a copy of the vault (replaced decisions marked superseded, learnings carried into the wiki, pages corrected where the log moved on, duplicates linked), checked and then applied as one commit the user can take back. Use in the scheduled weekly run (for example the Hermes weekly job, after the review), or when the user asks for it ("dream", "rüya geçişi", "haftalık toparlama", "vault'u toparla"). Also when the user wants (part of) a dream taken back ("dream'in şunu geri al", "undo the dream"), or the session context says a dream waits on its branch.
---

# kioku-dream

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions.
Rules: `$VAULT/.kioku/SCHEMA.md`; personal rules in `$VAULT/AGENTS.md`
CLI: `$VAULT/.kioku/bin/kioku`

## Run the pass

1. `kioku dream start` (a window of 7 days; `--days N` for another). If it prints `SKIP`, stop and say so in one line.
2. It prints `DREAM <folder>`: a copy of the vault on its own branch. Work **only in that folder**, never in the vault itself. Read its `.kioku/SCHEMA.md` and `AGENTS.md`, then the daily files of the window whole, and the pages they link to. The two check reports it printed are hints, not orders.
3. Change only what the vault itself makes clear:
   1. **Replaced decisions:** a decision or preference of the window that replaces an earlier one (of the same project, or plainly the same subject under another tag): change the older line's type to `superseded:` (keep its text) and end the newer line with `(replaces YYYY-MM-DD HH:MM)`. Two lines that only share a subject both stay; most pairs in the supersedes report are only related.
   2. **Learnings into the wiki:** a `learned:` line of the window without a `[[wiki/…]]` link: add the knowledge to the right concept or entity page with `(source: [[daily/YYYY-MM-DD]])` (create the page if there is none, with frontmatter, a Topics line and a hub entry), then append the page link to the daily line.
   3. **Pages behind the log:** a project, wiki or hub page that states something a newer line changed (a time, a setting, a version, a status, which tool is used): correct that sentence, cite the daily line, set `updated:`.
   4. **Duplicates:** the same knowledge on two wiki pages: keep it on the better page and link it from the other.
4. Do not touch `Core.md`, `AGENTS.md`, `raw/`, `.kioku/`, `journal/`, `calendar/`, `contacts/`, `reviews/` or `eval/`; do not delete, move or rename files or delete daily lines; do not resolve a contradiction you are not sure of or reword a page beyond the sentence that changed. (`finish` puts such changes back anyway.) Do not commit: `finish` does.
5. `kioku dream finish -m "<one line per change: file: what, why>"`. It puts back anything outside the rules and drops the pass when search (kioku eval) or the checks got worse; else it applies the pass to the vault as one commit (or, in review mode, leaves it on its branch).
6. Your final message, in the language of the user's notes, short: what `finish` said, the changes (one line each), then proposals: things for `Core.md` or `AGENTS.md`, and anything you were not sure of. In the weekly run this comes after the review.

## Taking a dream back

When the user does not want a change of the last dream: `kioku dream undo <file> …` (or `kioku dream undo` for all of it). Each file goes back to how it was before the pass, in a new commit. Say what was taken back.

## When a dream waits on its branch

Only with `git config kioku.dreamreview true` in the vault, or when the pass no longer applied cleanly. The session context says so. At a natural moment, tell the user in one line and offer to go through it. `kioku dream show` prints the branch's summary and diff: explain the changes in a few plain bullets (not the diff itself). Then, with their answer:
- all of it: `kioku dream merge`;
- some files: `kioku dream merge --only <file> …`;
- none: `kioku dream drop`.

A newer dream replaces one that was never merged.
