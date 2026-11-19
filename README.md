# kioku

**English** | [Türkçe](README.tr.md)

A personal second brain for AI agents: [Claude Code](https://claude.com/claude-code),
[Codex](https://developers.openai.com/codex), [Google Antigravity](https://antigravity.google),
[Hermes Agent](https://github.com/NousResearch/hermes-agent), and any other agent that can read
files and run commands. You talk, the agent keeps the notes.

- **Memory:** your agent remembers who you are, what you decided and what you're working on, across sessions and across projects.
- **Wiki:** links and PDFs you hand over are compiled into an interlinked wiki that keeps growing, following Andrej Karpathy's [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) pattern.
- **Journal:** your own notes, todos and ideas, taken from chat the moment you say them.

Everything is plain Markdown in a folder you own. You can open it as an
[Obsidian](https://obsidian.md) vault, and it is versioned with git. There is no server,
database or API key of its own; the CLI and the hooks are one small compiled program
(`kioku`, written in Rust). The same vault works with
every agent, so you can switch tools, or use several, and keep one memory.

## How it works

```
~/Documents/Kioku/            your vault (private, your own git repo)
  AGENTS.md                   points every agent to the schema + your personal rules
  CLAUDE.md                   two lines for Claude Code: imports AGENTS.md and the schema
  Core.md                     who you are: filled in by a "get to know me" interview
  daily/                      the agent's work log: decisions, results, learnings, open threads
  journal/                    your timeline: what you did, todos (with due dates), what you saved
  calendar/                   events, one page each
  contacts/                   people and their contact details
  projects/                   one note per project: status, next step, decisions
  reviews/                    weekly or monthly reviews
  raw/                        exact copies of sources you ingested (never edited)
  wiki/                       compiled pages: topics/ (category hubs) concepts/ entities/ ideas/ sources/ synthesis/
  index.md  log.md            map of the brain (topics first), append-only operation log
  Home.md                     start page for Obsidian, generated on each machine (not in git)
  .kioku → kioku/             link to this repo: schema, hooks, skills, CLI, templates
```

Three parts work with any agent:

- **Schema ([SCHEMA.md](SCHEMA.md)):** the rules the agent follows: what goes where, languages, citations, what never to write (secrets), commit style, untrusted content.
- **Skills** (standard `SKILL.md` folders):
  - `kioku` records memory and runs the onboarding interview;
  - `kioku-journal` keeps your timeline and todos;
  - `kioku-note` turns tools, sites, ideas and things you learned into linked wiki pages, categorized in topic hubs;
  - `kioku-calendar` and `kioku-contacts` keep events and contact details;
  - `kioku-ingest` adds sources;
  - `kioku-ask` answers with citations;
  - `kioku-lint` runs a health check of the notes;
  - `kioku-review` writes weekly reviews and challenges plans.
- **CLI (`kioku`):** context, search, todos, agenda, new pages, checks, git sync.

On top of that, the installer wires each agent it finds on the machine. What each one gets:

| | Claude Code | Codex | Antigravity | Hermes | Other agents |
| --- | --- | --- | --- | --- | --- |
| Rules | `~/.claude/CLAUDE.md` | `~/.codex/AGENTS.md` | `~/.gemini/GEMINI.md` | the vault's `.hermes.md` | the vault's `AGENTS.md` |
| Skills | `~/.claude/skills/` | `~/.agents/skills/` | `~/.gemini/config/skills/` | `skills.external_dirs` | point it at `.kioku/skills/` |
| Session context at start | hook | hook | hook | `kioku context` | `kioku context` |
| Pull before each prompt (autosync) | hook | hook | hook | — | — |
| Write check | hook | hook | — | — | — |
| Recording reminder at the end | hook | hook | hook | — | — |
| Note before compaction | hook | — | — | — | — |

The hooks:

- **Session start:** every session, in any project, starts with a compact context of about 1k tokens: `Core.md`, the note for the current folder, active projects, due todos, upcoming events, the last two work-log days and a wiki summary. Your journal stays out of it, except for due todos. Where there is no hook, the rules tell the agent to run `kioku context` itself.
- **Before each prompt:** with autosync on, the vault is pulled first, so notes written on another machine show up.
- **Write check:** right after the agent writes a vault file, its frontmatter, `Topics:` line, `[[links]]` and todos/events are checked, and the agent fixes what is wrong in the same turn.
- **Recording reminder:** the agent is asked to record, once each, in two cases:
  - a session did real work but recorded nothing;
  - a learning went into the work log but not into the wiki.

  Short Q&A sessions are left alone.
- **Before compaction:** when a long session is compacted, the summary lists what was not recorded yet, and the agent records it right after.

The schema's core idea comes from the LLM Wiki: instead of searching raw documents on every
question (RAG), the agent reads a source once and integrates it into concept and entity pages.
Contradictions are flagged and every claim keeps a link to its source.

## Requirements

- An AI agent that can read files and run shell commands (see the table above)
- `git`
- The `kioku` binary: the installer downloads it for Linux (x86_64, aarch64) and macOS, or builds it with `cargo` (the Nix package builds it too)
- Obsidian (optional): only for browsing

Developed and used on Linux (NixOS and Ubuntu). macOS builds are published but not tested yet. On Windows, use WSL.

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
    # claudeCode = false; codex = false; antigravity = false;   # to leave an agent alone
  };
}
```

The module works like this:
- **Command:** it puts `kioku` and `git` on your PATH.
- **Wiring:** on every switch, it runs `kioku install`. The vault's `.kioku` link points at the package in the Nix store, and the version is pinned in your `flake.lock`.
- **Updating:** run `nix flake update kioku`, then rebuild.

The agents' files (`~/.claude/settings.json`, `~/.codex/hooks.json`, `~/.codex/config.toml` …)
are also edited by the agents themselves. The module merges its entries into them, as
`install.sh` does, rather than owning the files.

**Working on Kioku itself:** set `programs.kioku.source = "/path/to/your/checkout";`. The vault
then links to that checkout, so your edits take effect without a rebuild. Remove the line to go
back to the pinned version.

**Removing:** disabling the module does not unwire the agents. Run `kioku-install --uninstall`
from the package first.

### Other systems

```bash
git clone https://github.com/Drum3X/kioku ~/Projects/kioku
```

```bash
~/Projects/kioku/install.sh
```

`install.sh` downloads the `kioku` binary for this version into `bin/` (from the GitHub release, sha256 checked), or builds it with `cargo` when there is no release for your platform (`KIOKU_BUILD=1` always builds), then runs `bin/kioku install`.

Keep the checkout where you cloned it: the vault links to it. The default vault is
`~/Documents/Kioku`; pass `--vault PATH` to choose another folder, or an existing Obsidian
vault (your notes are not touched). The installer:

1. creates the vault folders and starter files. Existing files are never overwritten.
2. links `vault/.kioku` to the checkout, and runs `git init` in the vault if it is not a repo yet.
3. links the `kioku` command into `~/.local/bin/` (it warns if that folder is not on your `PATH`).
4. wires every agent it finds (see [Agents](#agents)); `--no-claude`, `--no-codex`, `--no-antigravity` and `--no-hermes` leave one alone and remove what was set up there.

Re-running it is safe. Then open a new session in your agent and say **"get to know me"**.

## Agents

### Claude Code

When `~/.claude` exists or the `claude` command is found:

- **Rules:** a Kioku section in `~/.claude/CLAUDE.md`; the vault's own `CLAUDE.md` imports `AGENTS.md` and the schema.
- **Skills:** linked into `~/.claude/skills/`.
- **Hooks and permissions:** all five hooks, read/edit permissions for the vault and a sandbox write rule in `~/.claude/settings.json`. A backup is kept as `settings.json.kioku.bak`.

### Codex

When `~/.codex` (or `$CODEX_HOME`) exists or the `codex` command is found:

- **Rules:** a Kioku section in `~/.codex/AGENTS.md`. Inside the vault, Codex also reads the vault's `AGENTS.md` itself.
- **Skills:** linked into `~/.agents/skills/`, the shared folder that Codex and other agents read.
- **Hooks:** four entries in `~/.codex/hooks.json` (other hooks there are kept), run as `kioku hook codex …`:
  - `SessionStart` adds the session context;
  - `UserPromptSubmit` pulls the vault with autosync;
  - `PostToolUse` counts the session's tool calls and runs the write check on files changed by `apply_patch`;
  - `Stop` asks once to record, then pushes the vault.

  Codex runs hooks only after you trust them: in a new session, run `/hooks` once and trust Kioku's.
- **Sandbox:** the vault is added to `writable_roots` in `~/.codex/config.toml`, so Codex can write to it from any project. If that file already has a `[sandbox_workspace_write]` table, the installer leaves it and asks you to add the vault there yourself. Commands in Codex's sandbox have no network, so a commit made there is pushed by the `Stop` hook at the end of the turn.
- **Not available:** the note before compaction (Codex's `PreCompact` hook cannot add instructions to the summary).

Codex's transcript format is not a stable interface, so the recording reminder does not read it:
the tool hook keeps what it needs in `~/.local/state/kioku/codex/`.

### Google Antigravity

When `~/.gemini` exists or the `antigravity` command is found:

- **Rules:** a Kioku section in `~/.gemini/GEMINI.md` (your own text in that file is kept).
- **Skills:** in `~/.gemini/config/skills/`; say the same things or call them as `/kioku-journal` and so on.
- **Hooks:** three entries named `kioku-*` in `~/.gemini/config/hooks.json` (other hooks there are kept), run as `kioku hook antigravity …`, which translates Antigravity's input and transcript:
  - `SessionStart` adds the session context;
  - `PreInvocation` pulls the vault with autosync (at most every 30 s);
  - `Stop` asks once to record.
- **Not available:** the write check and the note before compaction: Antigravity's `PostToolUse` cannot send anything back to the agent, and it has no compaction hook.

If the hooks don't run, the rules tell the agent to load the context itself with `kioku context`.
The last input of each hook is kept in `~/.local/state/kioku/antigravity-*.json` for debugging.

### Hermes Agent

When the `hermes` command is found:

- **Rules:** a Kioku block in the vault's `.hermes.md`, which Hermes reads when it works in the vault.
- **Skills:** Kioku's skills are listed in Hermes's `skills.external_dirs`.
- **Bundled skills:** Hermes's own `obsidian` and `weekly-review-planning` skills are turned off, so they don't do Kioku's job by other rules.

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

### Other agents

Any agent that reads `AGENTS.md` (OpenCode, Cursor, Aider, Gemini CLI …) finds the rules when you
open it in the vault. From other projects, add a line to its global instructions file, for
example: *"My second brain is ~/Documents/Kioku. At the start of every conversation run
`~/Documents/Kioku/.kioku/bin/kioku context`; before writing there, read its AGENTS.md and
.kioku/SCHEMA.md."* Point the agent's skill folder at `~/Documents/Kioku/.kioku/skills/` if it
supports `SKILL.md` skills; otherwise it can read them as files. Everything else works the
same; only the hooks are missing.

Which model an agent uses is up to you; that choice, its cost and its terms are your
responsibility.

## Use

Talk normally, in any language. A few examples:

| You say | What happens |
| --- | --- |
| "get to know me" | An open-ended interview, one question at a time, until you say stop; then a `Core.md` draft for your approval |
| *(you finish a task with your agent)* | One line in `daily/` and the project note, then `Saved to Kioku: …` |
| "add this to the brain: https://…" | Saved to `raw/`, key points discussed, summary + concept/entity pages, index, log |
| "what do we know about X?" | Answer from the wiki with `[[citations]]`; says so when the brain has nothing |
| "let's continue with Config" / "where were we with the server?" | `kioku context --deep`: earlier decisions, open threads and recent work first, then the work |
| "note: finished the exam today" / "remind me Friday: submit homework" | A line in today's `journal/` (relative dates become real dates) |
| "what are my todos?" / "I did the homework" | Grouped list / the todo is ticked in place |
| "save this: Ghostty, a GPU terminal I want to try" / "idea: a small time-loop game" / "TIL: `nix why-depends` shows the dependency chain" | A wiki page each (`entities/`, `ideas/`, `concepts/`), filed under a topic hub such as *Linux › Tools*; a new hub is created and announced when nothing fits |
| "Friday 14:00 dentist" / "every Monday 10:00 class" / "cancel tomorrow's meeting" | A page in `calendar/`, shown in the agenda |
| "add Ahmet to my contacts, his number is …" | A page in `contacts/` |
| "how did this week go?" / "challenge this plan" | A review under `reviews/` / the plan checked against past decisions |
| "check the brain" | Lint report first, fixes only after your approval; asks whether a few old facts still hold |
| *(in a terminal or with a shortcut)* `kioku capture "call the bank tomorrow"` | Lands in `raw/inbox/` without a session; the next session offers to file it |

Changes to `Core.md` always need your approval. Everything else is recorded without asking,
and each write is a git commit in your vault. The agent never pushes, unless you turn on
`kioku remote` for several machines.

The CLI is also usable by hand as `kioku` (a link to `vault/.kioku/bin/kioku`):

```
kioku health | capture "<text>" | context [--agent NAME | --deep <project|topic>] | whoami | today | journal | todo [--soon] | agenda [--days N] [--past] | new <type> "<Title>" | log <op> "<Title>"
kioku search [--raw] [-n N] <term...> | recap [--days N] | stale [--all] | home [--print] | remote on|off|status|now|setup | update [--check] | version | lint-check
```

## Quick capture and troubleshooting

- **`kioku capture "text or link"`** drops a thought, a todo or a link into `raw/inbox/` without opening a session (`kioku capture --file F` for a file, `kioku capture -` for stdin). It is committed, so with autosync it reaches your other machines too. The next session says how many wait and offers to file them: notes go to the journal, wiki or calendar as if you had said them, links and files are ingested. For a keyboard shortcut, `kioku capture --gui` asks in a small dialog (kdialog or zenity); with the Nix module, `programs.kioku.capture.gui = true;` brings the tools and a "Kioku capture" desktop entry to bind.
- **`Home.md`** is a start page for browsing the vault in Obsidian: events of the coming week, due todos, active projects with their next step, open threads, pages changed this week, pages due for review and the inbox, each linked to its page. It is generated (at session start, after each commit and after pulls, or with `kioku home`) and kept out of git, so every machine has its own; don't edit it.
- **Who wrote it:** several agents and machines write to one vault, so every work-log line carries `agent@host` next to its project (`- 21:55 [brain · claude@nitro] done: …`) and every commit ends with `Kioku-Agent: claude@nitro`. The session context tells each agent its name; `kioku whoami` prints it (the host is the machine's short name, `KIOKU_HOST` overrides it).
- **`kioku context --deep <project or topic>`** gathers everything about one project or topic for picking work up again: the note, its decisions, open threads, learnings and recent work from the daily log (newest first), linked and linking pages, related todos and events. The agent runs it when you return to a project ("let's continue with Config"); the session context points to it when a session starts in a project's folder.
- **`kioku stale`** lists wiki and project pages whose facts have not been checked for a while: by default active projects after 60 days, tools, sites and services after 90, people and concepts after 180 (anime, games and the like never) (`review_every:` changes it per page, `reviewed:` records the last check). The session context counts them, `kioku lint-check` lists them without counting them as issues, and the agent asks you about a few at a time ("Is this still true?"), then sets `reviewed:` or corrects the page.
- **`kioku health`** checks everything Kioku depends on (tools, the vault link and rules, each agent's rules, skills and hooks, git and autosync, vault health) and says how to fix each problem. `--offline` skips the network check.

## Calendar, contacts and todos

Events (`calendar/`), contacts (`contacts/`) and journal todos are notes in the vault. The
session context and `kioku agenda` / `kioku todo --soon` show what is coming up. Kioku itself
sends nothing to other calendars and rings no reminders: if you want that, build it on top,
for example with a Hermes cron job (see [Hermes Agent](#hermes-agent)).

## Several machines and your phone (optional)

You can keep a copy of the vault on a second machine, such as an always-on server, and use it
from your phone, for example with Claude Code's [Remote Control](https://code.claude.com/docs/en/remote-control)
or with Hermes on Telegram or Discord.

1. Push the vault to a private git remote once: `git -C ~/Documents/Kioku push -u origin HEAD`.
2. On each machine, run `kioku remote on`. From then on:
   - every session starts with a `git pull`, and each prompt pulls again (at most every 30 s), so a session that is already open sees notes written elsewhere;
   - every commit is pulled, rebased and pushed in the background.
3. On the server: clone this repo and the vault, run `install.sh`, then `kioku remote on`; then start your agent there (for example `claude remote-control` inside the vault, in tmux or as a service).

When two machines append to the same daily or journal file, git keeps both sides (`merge=union`
in `.gitattributes`). A real conflict is never forced: the local commits stay, nothing is
pushed, and the next session tells you about it. Set `TZ` on a server that runs in UTC, so
"today" and event times match yours. `kioku remote status` shows the state; logs go to
`~/.local/state/kioku/git-sync.log`.

## Languages

System text is English: schema, skills, headings, entry types, frontmatter. Note content
follows its source: an article is summarized in its own language, and conversation notes are
written in the language you speak. Every page records its language in `lang:`. Search is ranked
(title, headings, rare terms and wiki pages count more), treats Turkish `İ/ı`, `I/i`, `ç/c`,
`ş/s` and the like as the same letter, and allows suffixes: "açılışta" finds "açılış".

## Configure

- **Personal rules:** tell your agent ("from now on, …"); it adds them under *Personal rules* in your vault's `AGENTS.md`, with your approval.
- **Private pages:** `private: true` keeps a page out of the session-start context.
- **Environment variables** for the hooks (in Claude Code, set them in `~/.claude/settings.json` under `env`; for other agents, in the environment they start from):
  - `KIOKU_STOP_CHECK=0` turns off the recording reminder and the compaction note.
  - `KIOKU_WRITE_CHECK=0` turns off the write check.
  - `KIOKU_STOP_MIN_TOOLS=10` sets how many tool calls count as "real work".
  - `KIOKU_CONTEXT_MAX=12000` caps the session-start context in bytes.

## Development

The source is in `src/` (one Rust crate, no network or async dependencies). With Nix, `nix develop`
gives cargo, clippy and rustfmt. `cargo build --release` builds `target/release/kioku`;
`install.sh` links `bin/kioku` to it when it builds from source, so a checkout the vault links to
runs your build. `cargo test` runs the unit tests; `tests/make-fixture.sh DIR` builds a test
vault with tricky cases (recurring and broken events, invalid dates, broken links, private
pages) to try commands on. A `v*` tag builds and publishes the release binaries
(`.github/workflows/release.yml`).

## Update and uninstall

```bash
kioku update
```

This pulls the checkout and re-runs the install step with the options you installed with,
then lists what changed. `kioku update --check` only looks for new commits, and
`kioku version` prints the installed version.

If you update with a plain `git pull` instead, the next session notices when the install step
is out of date (new skills, settings or migrations) and the agent offers to run `kioku update`.
With the Nix module, run `nix flake update kioku` and rebuild; the install step runs on every
switch.

```bash
~/Projects/kioku/install.sh --uninstall
```

This removes the `kioku` command link and, from every agent, the skills, the rules sections
and the hook entries. Your vault is kept.

## Privacy

Your notes stay on your machine and in your own git repository; what the agent reads from them
goes to the model provider of the agent you use, like the rest of your conversation. The schema
forbids writing secrets (passwords, keys, tokens, ID or card numbers). If you push your vault,
use a private repository.

## Credits

- [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) by Andrej Karpathy: the raw / wiki / schema layers and the ingest, query and lint operations.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): the idea of an agent-maintained personal memory with a core profile and daily records.

## License

[GPL-3.0](LICENSE)
