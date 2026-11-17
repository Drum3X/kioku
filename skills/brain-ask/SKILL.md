---
name: brain-ask
description: Searches the user's second brain (the Brain vault) and answers with citations. Use when the user asks about past knowledge, decisions or sources they've read — "is X in the brain", "what do we know about X", "what did we decide", "what did that article say" (Turkish: "beyinde X var mı", "X hakkında ne biliyoruz", "ne karar vermiştik").
argument-hint: "<question>"
---

# brain-ask

Vault: `$BRAIN` — the vault path from the Brain section of your instructions (`~/.claude/CLAUDE.md`).
Rules: `$BRAIN/.brain/SCHEMA.md` (Query section); personal rules in `$BRAIN/CLAUDE.md`
CLI: `$BRAIN/.brain/bin/brain`

1. **Search.** Read `$BRAIN/index.md` › Topics and the relevant topic hubs (`wiki/topics/`) first. Extract 1–3 key terms from the question and run `brain search <term>` for each. Pages may be in different languages, so search in the question's language and in the likely languages of the sources (for example the English term too). For decisions and past work, look at results in `daily/` and `projects/` as well.
2. **Read.** Read the 3–8 most relevant pages. If a wiki claim looks doubtful or you need detail, go down to the `raw/` file listed in `sources:`.
3. **Answer** briefly in the user's language, with the source next to every claim: `([[wiki/concepts/x]])`.
   - Say plainly what the brain doesn't have: "The brain has nothing on this."
   - If you add general knowledge, keep it separate and marked.
   - If records conflict, show both.
4. **Write back (optional).** If the answer has lasting value (a comparison, a synthesis, a long analysis), ask "Shall I save this to the wiki as a synthesis?". If yes:
   - create it with `brain new synthesis "<Title>"`, in the user's language;
   - add a line to `index.md` › Syntheses;
   - run `brain log query "<question>"`;
   - commit with `query: <title>`.
5. If a new decision or learning came up during the question, follow **Record** in the `brain` skill.
