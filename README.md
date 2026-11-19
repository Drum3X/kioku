# kioku

**English** | [Türkçe](README.tr.md)

A personal second brain for [Claude Code](https://claude.com/claude-code). You talk, Claude
keeps the notes.

- **Memory:** Claude remembers who you are, what you decided and what you're working on, across sessions and across projects.
- **Wiki:** links and PDFs you hand over are compiled into an interlinked wiki that keeps growing, following Andrej Karpathy's [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) pattern.
- **Journal:** your own notes, todos and ideas, taken from chat the moment you say them.

Everything is plain Markdown in a folder you own. You can open it as an
[Obsidian](https://obsidian.md) vault, and it is versioned with git. There is no server,
database or API key; the hooks are small bash scripts.

## How it works

```
~/Documents/Kioku/            your vault (private, your own git repo)
  CLAUDE.md                   imports the schema + your personal rules
  Core.md                     who you are: filled in by a "get to know me" interview
  daily/                      Claude's work log: decisions, results, learnings, open threads
  journal/                    your timeline: what you did, todos (with due dates), what you saved
  calendar/                   events, one page each
  contacts/                   people and their contact details
  projects/                   one note per project: status, next step, decisions
  raw/                        exact copies of sources you ingested (never edited)
  wiki/                       compiled pages: topics/ (category hubs) concepts/ entities/ ideas/ sources/ synthesis/
  index.md  log.md            map of the brain (topics first), append-only operation log
  .kioku → kioku/      link to this repo: schema, hooks, skills, CLI, templates
```

Claude Code is wired up in four places:

- **SessionStart hook:** every session, in any project, starts with a compact context of about 1k tokens: `Core.md`, the note for the current folder, active projects, due todos, the last two work-log days and a wiki summary. Your journal stays out of it, except for due todos.
- **Stop hook:** Claude is asked to record, once each, in two cases:
  - a session did real work but recorded nothing;
  - a learning went into the work log but not into the wiki.

  Short Q&A sessions are left alone.
- **Write check (PostToolUse):** right after Claude writes a vault file, its frontmatter, `Topics:` line, `[[links]]` and todos/events are checked, and Claude fixes what is wrong in the same turn.
- **Before compaction (PreCompact):** when a long session is compacted, the summary lists what was not recorded yet, and Claude records it right after.
- **Before each prompt (UserPromptSubmit):** with autosync on, the vault is pulled first, so notes written on another machine show up.
- **Skills:**
  - `kioku` records memory and runs the onboarding interview;
  - `kioku-journal` keeps your timeline and todos;
  - `kioku-note` turns tools, sites, ideas and things you learned into linked wiki pages, categorized in topic hubs;
  - `kioku-calendar` and `kioku-contacts` keep events and contact details;
  - `kioku-ingest` adds sources;
  - `kioku-ask` answers with citations;
  - `kioku-lint` runs a health check.
- **Schema ([SCHEMA.md](SCHEMA.md)):** the rules Claude follows: what goes where, languages, citations, what never to write (secrets), commit style.

The schema's core idea comes from the LLM Wiki: instead of searching raw documents on every
question (RAG), Claude reads a source once and integrates it into concept and entity pages.
Contradictions are flagged and every claim keeps a link to its source.

## Requirements

- Claude Code
- bash 4+, `jq`, `ripgrep`, `git` (the Nix package brings its own)
- `node` (optional): used to make file names from non-Latin titles (`машинное-обучение.md`, `機械学習.md`)
- Obsidian (optional): only for browsing

Developed and used on Linux (NixOS). macOS should work with a newer bash (`brew install bash jq ripgrep`). On Windows, use WSL. Neither macOS nor Windows has been tested yet.

## Install

### NixOS / Home Manager (flake)

Add the flake as an input and enable the Home Manager module:

```nix
# flake.nix
inputs.kioku = {
  url = "github:Drum3X/kioku";
  inputs.nixpkgs.follows = "nixpkgs";
};
```

```nix
# a Home Manager module
{config, inputs, ...}: {
  imports = [inputs.kioku.homeManagerModules.default];
  programs.kioku = {
    enable = true;
    vault = "${config.home.homeDirectory}/Documents/Kioku"; # the default
  };
}
```

The module works like this:
- **Command:** it puts `kioku` and its tools (`jq`, `ripgrep`, `git`, `node`) on your PATH.
- **Wiring:** on every switch, it runs the same steps as `install.sh`. The vault's `.kioku` link points at the package in the Nix store, and the version is pinned in your `flake.lock`.
- **Updating:** run `nix flake update kioku`, then rebuild.

`~/.claude/settings.json` and `~/.claude/CLAUDE.md` are also edited by Claude Code itself. The
module merges its entries into them, as `install.sh` does, rather than owning the files.

**Working on Kioku itself:** set `programs.kioku.source = "/path/to/your/checkout";`. The vault
then links to that checkout, so your edits take effect without a rebuild. Remove the line to go
back to the pinned version.

**Removing:** disabling the module does not unwire Claude Code. Run `kioku-install --uninstall`
from the package first.

### Other systems

```bash
git clone https://github.com/Drum3X/kioku ~/Projects/kioku
```

```bash
~/Projects/kioku/install.sh
```

Keep the checkout where you cloned it: the vault links to it. The default vault is
`~/Documents/Kioku`; pass `--vault PATH` to choose another folder, or an existing Obsidian
vault (your notes are not touched). The installer:

1. creates the vault folders and starter files. Existing files are never overwritten.
2. links `vault/.kioku` to the checkout, and runs `git init` in the vault if it is not a repo yet.
3. links the skills into `~/.claude/skills/`, and the `kioku` command into `~/.local/bin/` (it warns if that folder is not on your `PATH`).
4. adds a short Kioku section to `~/.claude/CLAUDE.md`.
5. adds the hooks, read/edit permissions for the vault and a sandbox write rule to `~/.claude/settings.json`. A backup is kept as `settings.json.kioku.bak`.

Re-running it is safe. Then open a new Claude Code session and say **"get to know me"**.

## Use

Talk normally, in any language. A few examples:

| You say | What happens |
| --- | --- |
| "get to know me" | An open-ended interview, one question at a time, until you say stop; then a `Core.md` draft for your approval |
| *(you finish a task with Claude)* | One line in `daily/` and the project note, then `Saved to Kioku: …` |
| "add this to the brain: https://…" | Saved to `raw/`, key points discussed, summary + concept/entity pages, index, log |
| "what do we know about X?" | Answer from the wiki with `[[citations]]`; says so when the brain has nothing |
| "note: finished the exam today" / "remind me Friday: submit homework" | A line in today's `journal/` (relative dates become real dates) |
| "what are my todos?" / "I did the homework" | Grouped list / the todo is ticked in place |
| "save this: Ghostty, a GPU terminal I want to try" / "idea: a small time-loop game" / "TIL: `nix why-depends` shows the dependency chain" | A wiki page each (`entities/`, `ideas/`, `concepts/`), filed under a topic hub such as *Linux › Tools*; a new hub is created and announced when nothing fits |
| "Friday 14:00 dentist" / "every Monday 10:00 class" / "cancel tomorrow's meeting" | A page in `calendar/`, shown in the agenda |
| "add Ahmet to my contacts, his number is …" | A page in `contacts/` |
| "what did I do this week?" | `kioku recap` summary |
| "check the brain" | Lint report first, fixes only after your approval |
| *(in a terminal or with a shortcut)* `kioku capture "call the bank tomorrow"` | Lands in `raw/inbox/` without a session; the next session offers to file it |

Changes to `Core.md` always need your approval. Everything else is recorded without asking,
and each write is a git commit in your vault. Claude never pushes, unless you turn on `kioku remote` for several machines.

The CLI is also usable by hand as `kioku` (a link to `vault/.kioku/bin/kioku`):

```
kioku health | capture "<text>" | context | today | journal | todo [--soon] | agenda [--days N] [--past] | new <type> "<Title>" | log <op> "<Title>"
kioku search [--raw] [-n N] <term...> | recap [--days N] | remote on|off|status|now|setup | update [--check] | version | lint-check
```

## Quick capture and troubleshooting

- **`kioku capture "text or link"`** drops a thought, a todo or a link into `raw/inbox/` without opening a session (`kioku capture --file F` for a file, `kioku capture -` for stdin). It is committed, so with autosync it reaches your other machines too. The next session says how many wait and offers to file them: notes go to the journal, wiki or calendar as if you had said them, links and files are ingested. For a keyboard shortcut, `kioku capture --gui` asks in a small dialog (kdialog or zenity); with the Nix module, `programs.kioku.capture.gui = true;` brings the tools and a "Kioku capture" desktop entry to bind.
- **`kioku health`** checks everything Kioku depends on (tools, the vault link, hooks, skills, Antigravity, git and autosync, vault health) and says how to fix each problem. `--offline` skips the network check.

## Calendar, contacts and todos

Events (`calendar/`), contacts (`contacts/`) and journal todos are notes in the vault. The
session context and `kioku agenda` / `kioku todo --soon` show what is coming up. Kioku itself
sends nothing to other calendars and rings no reminders: if you want that, build it on top,
for example with a Hermes cron job (see below).
## Several machines and your phone (optional)

You can keep a copy of the vault on a second machine, such as an always-on server, and talk
to Claude there from your phone with [Remote Control](https://code.claude.com/docs/en/remote-control).

1. Push the vault to a private git remote once: `git -C ~/Documents/Kioku push -u origin HEAD`.
2. On each machine, run `kioku remote on`. From then on:
   - every session starts with a `git pull`, and each prompt pulls again (at most every 30 s), so a session that is already open sees notes written elsewhere;
   - every commit is pulled, rebased and pushed in the background.
3. On the server:
   - clone this repo and the vault, run `install.sh`, then `kioku remote on`;
   - start `claude remote-control` inside the vault, in tmux or as a service;
   - in the Claude app, the server appears under Code.

When two machines append to the same daily or journal file, git keeps both sides (`merge=union`
in `.gitattributes`). A real conflict is never forced: the local commits stay, nothing is
pushed, and the next session tells you about it. Set `TZ` on a server that runs in UTC, so
"today" and event times match yours. `kioku remote status` shows the state; logs go to
`~/.local/state/kioku/git-sync.log`.

## Hermes Agent (optional)

If [Hermes Agent](https://github.com/NousResearch/hermes-agent) is installed (the `hermes` command exists), the installer also connects Kioku to it:

- **Rules:** a Kioku block in the vault's `.hermes.md`, which Hermes reads when it works in the vault.
- **Skills:** Kioku's skills are listed in Hermes's `skills.external_dirs`.
- **Bundled skills:** Hermes's own `obsidian` and `weekly-review-planning` skills are turned off, so they don't do Kioku's job by other rules.

`--no-hermes` turns this off and removes it.

Kioku has no scheduler of its own. With Hermes you can add your own jobs that work in the vault
and send the result to Telegram, Discord and so on. Some examples:

```bash
# A morning brief without a model call: a script whose output Hermes delivers as is
cat > ~/.hermes/scripts/morning.sh <<'EOF'
#!/usr/bin/env bash
cd ~/Documents/Kioku && .kioku/bin/kioku remote pull
echo "Today:"; .kioku/bin/kioku agenda --days 1
echo; echo "Due soon:"; .kioku/bin/kioku todo --soon
EOF
chmod +x ~/.hermes/scripts/morning.sh
hermes cron create "0 8 * * *" --name morning --no-agent --script morning.sh --deliver telegram

# A weekly review with the kioku-review skill
hermes cron create "0 20 * * 0" "Weekly review of this vault with the kioku-review skill: write the page, commit it, then run .kioku/bin/kioku remote now. Reply with the review." \
  --name weekly --skill kioku-review --workdir ~/Documents/Kioku --deliver telegram
```

Which model Hermes uses is up to you (`hermes model`); that choice, its cost and its terms are your responsibility.
## Google Antigravity (optional)

If Antigravity is installed (`~/.gemini` exists), the installer also sets Kioku up there:

- **Rules:** a Kioku block in `~/.gemini/GEMINI.md` (your own text in that file is kept), like the one in `~/.claude/CLAUDE.md`.
- **Skills:** the same skills in `~/.gemini/config/skills/`; say the same things or call them as `/kioku-journal` and so on.

- **Hooks:** three entries named `kioku-*` in `~/.gemini/config/hooks.json` (other hooks there are kept). They run the same hooks as in Claude Code through `hooks/antigravity.sh`, which translates Antigravity's input and transcript:
  - `SessionStart` adds the session context;
  - `PreInvocation` pulls the vault with autosync (at most every 30 s);
  - `Stop` asks once to record, as in Claude Code.
  - The write check and the compaction note are Claude Code only: Antigravity's `PostToolUse` cannot send anything back to the agent, and it has no compaction hook.

If the hooks don't run, the rules tell the agent to load the context itself with `kioku context`.
The last input of each hook is kept in `~/.local/state/kioku/antigravity-*.json` for debugging.

Turn it off with `install.sh --no-antigravity` (Nix: `programs.kioku.antigravity = false;`); that also removes what was set up.

## Languages

System text is English: schema, skills, headings, entry types, frontmatter. Note content
follows its source: an article is summarized in its own language, and conversation notes are
written in the language you speak. Every page records its language in `lang:`. Search is ranked
(title, headings, rare terms and wiki pages count more), treats Turkish `İ/ı`, `I/i`, `ç/c`,
`ş/s` and the like as the same letter, and allows suffixes: "açılışta" finds "açılış".

## Configure

- **Personal rules:** tell Claude ("from now on, …"); it adds them under *Personal rules* in your vault's `CLAUDE.md`, with your approval.
- **Private pages:** `private: true` keeps a page out of the session-start context.
- **Environment variables**, set in `~/.claude/settings.json` under `env`:
  - `KIOKU_STOP_CHECK=0` turns off the Stop hook and the compaction note.
  - `KIOKU_WRITE_CHECK=0` turns off the write check.
  - `KIOKU_STOP_MIN_TOOLS=10` sets how many tool calls count as "real work".
  - `KIOKU_CONTEXT_MAX=12000` caps the session-start context in bytes.

## Update and uninstall

```bash
kioku update
```

This pulls the checkout and re-runs the install step with the options you installed with,
then lists what changed. `kioku update --check` only looks for new commits, and
`kioku version` prints the installed version.

If you update with a plain `git pull` instead, the next session notices when the install step
is out of date (new skills, settings or migrations) and Claude offers to run `kioku update`.
With the Nix module, run `nix flake update kioku` and rebuild; the install step runs on every
switch.

```bash
~/Projects/kioku/install.sh --uninstall
```

This removes the skills, the `kioku` command link, the `~/.claude/CLAUDE.md` section and the
settings entries. Your vault is kept.

## Privacy

Everything stays on your machine and in your own git repository. The schema forbids writing
secrets (passwords, keys, tokens, ID or card numbers). If you push your vault, use a private
repository.

## Credits

- [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) by Andrej Karpathy: the raw / wiki / schema layers and the ingest, query and lint operations.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): the idea of a Claude-maintained personal memory with a core profile and daily records.

## License

[GPL-3.0](LICENSE)
