---
name: kioku-review
description: Reviews a period of the user's life and work from their second brain (the Kioku vault) — what got done, what is stuck, decisions, learnings, what comes next, patterns — and saves it under reviews/; also challenges a plan against past decisions. Use when the user asks "how did this week go", "weekly review", "what did I do this month", "challenge this plan", "is this a good idea given what we decided" (Turkish: "bu hafta nasıl geçti", "haftalık değerlendirme", "bu ay ne yaptım", "bu planı sorgula", "şeytanın avukatlığını yap"), and in a scheduled weekly run (for example a Hermes cron job). For a plain list of the last days' entries, kioku recap is enough.
argument-hint: "[week | month | N days | challenge <plan>]"
---

# kioku-review

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (for example `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md` or `~/.gemini/GEMINI.md`).
Rules: `$VAULT/.kioku/SCHEMA.md` ("Reviews" and "Writing rules"); personal rules in `$VAULT/AGENTS.md`
CLI: `$VAULT/.kioku/bin/kioku`

A review **interprets**: it does not repeat the log line by line (that is `kioku recap`), it says
what the period amounted to. Every statement rests on a note and links to it. Write in the
language of the user's notes; keep it short (a phone screen or two).

## Review a period

1. **Period.** Default: the last 7 days ending today. "This month": from the 1st. "N days": N.
2. **Read.**
   - `kioku recap --days N`: work log (`daily/`) and the user's timeline (`journal/`) of the period.
   - Todos: `kioku todo` (open, with dates); in the period's journal pages, the ones ticked `[x]` or cancelled `[-]`.
   - What is coming: `kioku agenda --days 7`.
   - Active projects (`projects/*.md` with `status: active`): their status and `next:`.
   - New or updated wiki pages: `git log --since="N days ago" --name-only --format= -- wiki | sort -u`.
3. **Write** with `kioku new review "<Title>"` (title: `Week 2026-W47`, `November 2026`, or the dates), then fill it in. Set `period:` (week, month or days), `from:`, `to:` and `lang:`. Sections, each a few bullets, left out when empty:
   - **Done:** finished work and results that matter, grouped by project or area, not every small task.
   - **Stuck or moving slowly:** todos overdue or pushed again and again, projects without progress, open threads older than a week. Say how long.
   - **Decisions:** the period's decisions, one line each with the reason.
   - **Learned:** the durable learnings (link the wiki pages).
   - **Next:** due todos and events of the coming week, the projects' next steps, and at most three suggestions of your own, marked as suggestions.
   - **Patterns:** only if the notes really show one (the same problem twice, work piling up on one day, a todo that keeps moving). No psychology, no guessing.
   Link the sources: `[[daily/2026-11-19]]`, `[[projects/x]]`, `[[journal/2026-11-18]]`.
4. **Record.** List it in `index.md` › Reviews (create the section if missing), `kioku log query "<Title>"`, commit with `review: <title>`.
5. **Answer** in chat with the review's essence; in a scheduled run (for example a Hermes cron job), your final message is the review text itself.

Never invent: if the notes are thin, say so ("little was recorded this week") instead of filling
sections. Private pages (`private: true`) are read but only summarized in one neutral line.

## Challenge a plan

When the user asks to question a plan or a choice ("challenge", "devil's advocate", "is this a good idea"):

1. Search the brain for related decisions, learnings and failed attempts (`kioku search`, `projects/`, `daily/`).
2. Argue against the plan honestly: the strongest 2–4 objections, each grounded in a note (a past decision, a learned constraint, a stated preference in Core.md) or clearly marked as general reasoning.
3. Then say what would make the plan hold up, and which objection matters most.
4. If a new decision comes out of it, record it (kioku skill, Record).
