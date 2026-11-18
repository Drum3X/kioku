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
  calendar/                   events, one page each (synced to your CalDAV calendar)
  contacts/                   people and their contact details (synced to your CardDAV address book)
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
| "Friday 14:00 dentist" / "every Monday 10:00 class" / "cancel tomorrow's meeting" | A page in `calendar/`, synced to your calendar |
| "add Ahmet to my contacts, his number is …" | A page in `contacts/`, synced to your address book |
| "what did I do this week?" | `kioku recap` summary |
| "check the brain" | Lint report first, fixes only after your approval |
| *(in a terminal or with a shortcut)* `kioku capture "call the bank tomorrow"` | Lands in `raw/inbox/` without a session; the next session offers to file it |

Changes to `Core.md` always need your approval. Everything else is recorded without asking,
and each write is a git commit in your vault. Claude never pushes, unless you turn on `kioku remote` for several machines.

The CLI is also usable by hand as `kioku` (a link to `vault/.kioku/bin/kioku`):

```
kioku agent <job>|setup|status | notify "<subject>" | doctor | capture "<text>" | context | today | journal | todo [--soon] | agenda [--days N] [--past] | new <type> "<Title>" | log <op> "<Title>"
kioku search [--raw] <term> | recap [--days N] | sync [--check|--dry-run|--full] | remote on|off|status|now | password status|set|move|remove | update [--check] | version | lint-check
```

## Quick capture and troubleshooting

- **`kioku capture "text or link"`** drops a thought, a todo or a link into `raw/inbox/` without opening a session (`kioku capture --file F` for a file, `kioku capture -` for stdin). It is committed, so with autosync it reaches your other machines too. The next session says how many wait and offers to file them: notes go to the journal, wiki or calendar as if you had said them, links and files are ingested. For a keyboard shortcut, `kioku capture --gui` asks in a small dialog (kdialog or zenity); with the Nix module, `programs.kioku.capture.gui = true;` brings the tools and a "Kioku capture" desktop entry to bind.
- **`kioku doctor`** checks everything Kioku depends on (tools, the vault link, hooks, skills, Antigravity, git and autosync, the CalDAV/CardDAV login, vault health) and says how to fix each problem. `--offline` skips the network checks.

## Calendar, contacts and todos (optional sync)

Events (`calendar/`), contacts (`contacts/`) and journal todos can be pushed to any
CalDAV/CardDAV server, such as SOGo/mailcow, Nextcloud, Radicale or Fastmail, so you see them
in Thunderbird, on your phone or in webmail. The sync is **one way**: you tell Claude, the
vault changes, and a git hook pushes the change after the commit.

- **What it touches:** only resources named `kioku-…` in the chosen calendar and address book. Your other events and contacts are never touched.
- **Edits elsewhere:** if you edit a Kioku item in another client, the next sync overwrites it with the vault's version.

Setup:

1. Create an app password for your account that allows only CalDAV/CardDAV, if your server supports it.
2. Create `~/.config/kioku/sync.conf`:

   ```ini
   # the server's DAV root (/.well-known/caldav redirects there)
   url=https://mail.example.com/SOGo/dav/
   user=you@example.com
   # collection name or display name
   calendar=personal
   addressbook=personal
   ```

3. Store the password. Type it yourself, in a terminal:

   ```bash
   kioku password set
   ```

   It asks for the password twice without showing it, checks the login, and saves it only if
   the login works:
   - **Desktop:** it goes to the system keyring (Secret Service on Linux, such as KDE Wallet or
     GNOME Keyring; Keychain on macOS).
   - **No keyring** (a server, WSL): it goes to `~/.config/kioku/dav.pass` with `chmod 600`.
     The sync refuses to run if that file is readable by others.

   Other commands:
   - `--store keyring|file` picks the place yourself.
   - `kioku password status` shows where the password is, never the password itself.
   - `kioku password move keyring|file` moves it.
   - `kioku password remove` deletes it.

   `set`, `move` and `remove` only run in a real terminal, so no script or agent can feed them a
   password. Other password managers still work: set `password_cmd=` to any command that
   prints the password, such as `pass show dav` or `op read op://…`.

4. Test the connection; this only reads:

   ```bash
   ~/Documents/Kioku/.kioku/bin/kioku sync --check
   ```

5. Subscribe to the same calendar and address book in Thunderbird, and on your phone: DAVx⁵ on Android (with Tasks.org or jtx Board for todos), or a CalDAV/CardDAV account on iOS (todos appear in Reminders).

**Reminders** reach your devices as notifications:
- **Events:** "remind me 30 minutes before" adds `alarm: 30m` to the event.
- **Todos with a time:** a todo with a time (`(at: 17:00)`) rings at that time.
- **Earlier reminders:** `(remind: 1h)` rings earlier; `30m`, `1d` or `0` also work. Without a time, the reminder counts back from 09:00 on the due date.

The installer adds the vault's git hooks: `pre-commit` gives new items stable ids and
`post-commit` runs the sync in the background. Logs go to
`~/.local/state/kioku/sync.log`. Run `kioku sync --dry-run` to see what would be sent,
or `kioku sync --full` to resend everything. Sync needs `node`.

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

## Scheduled jobs (optional)

On a machine that is always on, such as the server of the previous section, Kioku can work on a
schedule and send you the result:

| Job | Default time | What it does | Model call |
| --- | --- | --- | --- |
| `morning` | every day 08:00 | Today's events and due todos, plus inbox items and sync problems | None |
| `nightly` | every day 03:00 | `kioku lint-check`; if it finds issues, Claude fixes the mechanical ones and reports the rest | Only when there are issues |
| `weekly` | Sunday 20:00 | A review of the week (`kioku-review`): done, stuck, decisions, learned, next; saved under `reviews/` | Once a week |

The unattended runs (`claude -p`) may only read, edit files in the vault, run the kioku CLI and
`git add/commit`; everything else is refused, and they never touch `Core.md`.

1. Settings go in `~/.config/kioku/agents.conf` (no secrets):
   ```ini
   notify=mail                # mail | command | stdout
   mail_to=you@example.com    # default: user= from sync.conf
   smtp=smtp://127.0.0.1:25   # the default: a mail server on the same machine
   lang=en                    # en | tr: the morning brief's language
   # morning_at=*-*-* 08:00   nightly_at=…   weekly_at=Sun *-*-* 20:00   weekly=off
   ```
   For Telegram, Discord or ntfy, set `notify=command` and `notify_command=/path/to/script`: the script gets the subject in `$KIOKU_SUBJECT` and the text on stdin.
2. Try a job: `kioku agent morning --dry-run` prints instead of sending; `kioku agent morning` sends.
3. `kioku agent setup` installs systemd user timers (on a server, enable lingering: `sudo loginctl enable-linger $USER`). `kioku agent status` shows the next runs and the log, `kioku agent remove` takes them away.

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
written in the language you speak. Every page records its language in `lang:`. Search treats
Turkish `İ/ı` and `I/i` as the same letter.

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
