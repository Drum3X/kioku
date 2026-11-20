---
name: kioku-ask
description: Searches the user's second brain (the Kioku vault) and answers with citations. Use when the user asks about past knowledge, decisions or sources they've read — "is X in the brain", "what do we know about X", "what did we decide", "what did that article say" (Turkish: "beyinde X var mı", "X hakkında ne biliyoruz", "ne karar vermiştik").
argument-hint: "<question>"
---

# kioku-ask

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (for example `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md` or `~/.gemini/GEMINI.md`).
Rules: `$VAULT/.kioku/SCHEMA.md` (Query section); personal rules in `$VAULT/AGENTS.md`
CLI: `$VAULT/.kioku/bin/kioku`

1. **Search.** For a question about one project or topic as a whole ("where were we with X", "why did we choose …"), start with `kioku context --deep X`. Otherwise read `$VAULT/index.md` › Topics and the relevant topic hubs (`wiki/topics/`) first. Extract the 2–5 key terms from the question and run one `kioku search <term> <term> …` with all of them; results are ranked, and a page does not need every term. Quote a term that must match as a phrase (`"dark mode"`). Pages may be in different languages, so add the likely terms of the sources' languages to the same command (for example the English term too), or run a second search if the first finds nothing useful. For decisions and past work, look at results in `daily/` and `projects/` as well.
2. **Read.** Read the 3–8 most relevant pages. If a wiki claim looks doubtful or you need detail, go down to the `raw/` file listed in `sources:`.
   If the right page turned up only after another search with other words, add the user's words to that page's `keys:` (and set `updated`), so the next search finds it at once.
3. **Answer** briefly in the user's language, with the source next to every claim: `([[wiki/concepts/x]])`.
   - Say plainly what the brain doesn't have: "The brain has nothing on this."
   - If you add general knowledge, keep it separate and marked.
   - If records conflict, show both.
4. **Write back.** Research the user asked for ("araştır", "araştırma yap", "research X") is saved without asking, before your final answer (SCHEMA.md, Query). For other answers with lasting value (a comparison, a synthesis, a long analysis), ask "Shall I save this to the wiki as a synthesis?". To save:
   - create it with `kioku new synthesis "<Title>"`, in the user's language, with `keys:` (SCHEMA.md, "Search keys");
   - add a line to `index.md` › Syntheses;
   - run `kioku log query "<question>"`;
   - commit with `query: <title>`.
5. If a new decision or learning came up during the question, follow **Record** in the `kioku` skill.
