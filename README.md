# second-brain

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
~/Documents/Brain/            your vault (private, your own git repo)
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
  .brain → second-brain/      link to this repo: schema, hooks, skills, CLI, templates
```

Claude Code is wired up in four places:

- **SessionStart hook:** every session, in any project, starts with a compact context of about 1k tokens: `Core.md`, the note for the current folder, active projects, due todos, the last two work-log days and a wiki summary. Your journal stays out of it, except for due todos.
- **Stop hook:** if a session did real work but recorded nothing, Claude is asked once to record what matters. Short Q&A sessions are left alone.
- **Skills:**
  - `brain` records memory and runs the onboarding interview;
  - `brain-journal` keeps your timeline and todos;
  - `brain-note` turns tools, sites, ideas and things you learned into linked wiki pages, categorized in topic hubs;
  - `brain-calendar` and `brain-contacts` keep events and contact details;
  - `brain-ingest` adds sources;
  - `brain-ask` answers with citations;
  - `brain-lint` runs a health check.
- **Schema ([SCHEMA.md](SCHEMA.md)):** the rules Claude follows: what goes where, languages, citations, what never to write (secrets), commit style.

The schema's core idea comes from the LLM Wiki: instead of searching raw documents on every
question (RAG), Claude reads a source once and integrates it into concept and entity pages.
Contradictions are flagged and every claim keeps a link to its source.

## Requirements

- Claude Code
- bash 4+, `jq`, `ripgrep`, `git`
- `node` (optional): used to make file names from non-Latin titles (`машинное-обучение.md`, `機械学習.md`)
- Obsidian (optional): only for browsing

Developed and used on Linux (NixOS). macOS should work with a newer bash (`brew install bash jq ripgrep`). On Windows, use WSL. Neither macOS nor Windows has been tested yet.

## Install

```bash
git clone https://github.com/Drum3X/second-brain ~/Projects/second-brain
```

```bash
~/Projects/second-brain/install.sh
```

Keep the checkout where you cloned it: the vault links to it. The default vault is
`~/Documents/Brain`; pass `--vault PATH` to choose another folder, or an existing Obsidian
vault (your notes are not touched). The installer:

1. creates the vault folders and starter files. Existing files are never overwritten.
2. links `vault/.brain` to the checkout, and runs `git init` in the vault if it is not a repo yet.
3. links the skills into `~/.claude/skills/`.
4. adds a short Brain section to `~/.claude/CLAUDE.md`.
5. adds the hooks, read/edit permissions for the vault and a sandbox write rule to `~/.claude/settings.json`. A backup is kept as `settings.json.brain.bak`.

Re-running it is safe. Then open a new Claude Code session and say **"get to know me"**.

## Use

Talk normally, in any language. A few examples:

| You say | What happens |
| --- | --- |
| "get to know me" | An open-ended interview, one question at a time, until you say stop; then a `Core.md` draft for your approval |
| *(you finish a task with Claude)* | One line in `daily/` and the project note, then `Saved to Brain: …` |
| "add this to the brain: https://…" | Saved to `raw/`, key points discussed, summary + concept/entity pages, index, log |
| "what do we know about X?" | Answer from the wiki with `[[citations]]`; says so when the brain has nothing |
| "note: finished the exam today" / "remind me Friday: submit homework" | A line in today's `journal/` (relative dates become real dates) |
| "what are my todos?" / "I did the homework" | Grouped list / the todo is ticked in place |
| "save this: Ghostty, a GPU terminal I want to try" / "idea: a small time-loop game" / "TIL: `nix why-depends` shows the dependency chain" | A wiki page each (`entities/`, `ideas/`, `concepts/`), filed under a topic hub such as *Linux › Tools*; a new hub is created and announced when nothing fits |
| "Friday 14:00 dentist" / "every Monday 10:00 class" / "cancel tomorrow's meeting" | A page in `calendar/`, synced to your calendar |
| "add Ahmet to my contacts, his number is …" | A page in `contacts/`, synced to your address book |
| "what did I do this week?" | `brain recap` summary |
| "check the brain" | Lint report first, fixes only after your approval |

Changes to `Core.md` always need your approval. Everything else is recorded without asking,
and each write is a git commit in your vault. Claude never pushes; that is up to you.

The CLI is also usable by hand, from `vault/.brain/bin/brain`:

```
brain today | journal | todo [--soon] | agenda [--days N] | new <type> "<Title>" | log <op> "<Title>"
brain search [--raw] <term> | recap [--days N] | sync [--check|--dry-run|--full] | lint-check
```

## Calendar, contacts and todos (optional sync)

Events (`calendar/`), contacts (`contacts/`) and journal todos can be pushed to any
CalDAV/CardDAV server, such as SOGo/mailcow, Nextcloud, Radicale or Fastmail, so you see them
in Thunderbird, on your phone or in webmail. The sync is **one way**: you tell Claude, the
vault changes, and a git hook pushes the change after the commit.

- **What it touches:** only resources named `brain-…` in the chosen calendar and address book. Your other events and contacts are never touched.
- **Edits elsewhere:** if you edit a Brain item in another client, the next sync overwrites it with the vault's version.

Setup:

1. Create an app password for your account that allows only CalDAV/CardDAV, if your server supports it.
2. Create `~/.config/second-brain/sync.conf`:

   ```ini
   # the server's DAV root (/.well-known/caldav redirects there)
   url=https://mail.example.com/SOGo/dav/
   user=you@example.com
   # collection name or display name
   calendar=personal
   addressbook=personal
   ```

3. Put the password, and nothing else, in `~/.config/second-brain/dav.pass` and make it readable only by you:

   ```bash
   chmod 600 ~/.config/second-brain/dav.pass
   ```

   The sync refuses to run if the file is readable by others. This works the same on Linux, macOS and WSL.

   To use a keychain instead, set `password_cmd=` to any command that prints the password. Examples:
   - Linux: `secret-tool lookup service second-brain`
   - macOS: `security find-generic-password -s second-brain -w`
   - any OS: `pass show dav`
   - 1Password: `op read op://…`

4. Test the connection; this only reads:

   ```bash
   ~/Documents/Brain/.brain/bin/brain sync --check
   ```

5. Subscribe to the same calendar and address book in Thunderbird, and on your phone: DAVx⁵ on Android (with Tasks.org or jtx Board for todos), or a CalDAV/CardDAV account on iOS (todos appear in Reminders).

**Reminders** reach your devices as notifications:
- **Events:** "remind me 30 minutes before" adds `alarm: 30m` to the event.
- **Todos with a time:** a todo with a time (`(at: 17:00)`) rings at that time.
- **Earlier reminders:** `(remind: 1h)` rings earlier; `30m`, `1d` or `0` also work. Without a time, the reminder counts back from 09:00 on the due date.

The installer adds the vault's git hooks: `pre-commit` gives new items stable ids and
`post-commit` runs the sync in the background. Logs go to
`~/.local/state/second-brain/sync.log`. Run `brain sync --dry-run` to see what would be sent,
or `brain sync --full` to resend everything. Sync needs `node`.

## Languages

System text is English: schema, skills, headings, entry types, frontmatter. Note content
follows its source: an article is summarized in its own language, and conversation notes are
written in the language you speak. Every page records its language in `lang:`. Search treats
Turkish `İ/ı` and `I/i` as the same letter.

## Configure

- **Personal rules:** tell Claude ("from now on, …"); it adds them under *Personal rules* in your vault's `CLAUDE.md`, with your approval.
- **Private pages:** `private: true` keeps a page out of the session-start context.
- **Environment variables**, set in `~/.claude/settings.json` under `env`:
  - `BRAIN_STOP_CHECK=0` turns off the Stop hook.
  - `BRAIN_STOP_MIN_TOOLS=10` sets how many tool calls count as "real work".
  - `BRAIN_CONTEXT_MAX=12000` caps the session-start context in bytes.

## Update and uninstall

```bash
git -C ~/Projects/second-brain pull
```

```bash
~/Projects/second-brain/install.sh
```

Hooks, skills and the schema update with `git pull`, because the vault links to the checkout.
Re-running the installer only matters when the installer itself changed.

```bash
~/Projects/second-brain/install.sh --uninstall
```

This removes the skills, the `~/.claude/CLAUDE.md` section and the settings entries. Your
vault is kept.

## Privacy

Everything stays on your machine and in your own git repository. The schema forbids writing
secrets (passwords, keys, tokens, ID or card numbers). If you push your vault, use a private
repository.

## Credits

- [LLM Wiki](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f) by Andrej Karpathy: the raw / wiki / schema layers and the ingest, query and lint operations.
- [avenox Beyin](https://github.com/avenoxai/avenoxbeyin): the idea of a Claude-maintained personal memory with a core profile and daily records.

## License

[GPL-3.0](LICENSE)
