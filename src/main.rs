//! kioku: the CLI, the agents' hooks and the installer of the Kioku second brain.

mod calendar;
mod context;
mod deep;
mod fm;
mod git;
mod health;
mod home;
mod hook;
mod install;
mod lint;
mod notes;
mod search;
mod stale;
mod text;
mod update;
mod util;
mod version;

use anyhow::Result;
use std::path::PathBuf;

const USAGE: &str = r#"Usage: kioku <command> [args]
  context                       Print the session-start context (Core, recent notes, todos, agenda);
                                pulls first with autosync. For agents without a SessionStart hook
  context --agent NAME          The same, signing records as NAME@host (the agent: claude, codex, hermes …)
  whoami [--agent NAME]         Who writes the records: NAME@host (default $KIOKU_AGENT)
  context --deep <project|topic>  Everything about one project or topic: the note, decisions, open
                                threads, learnings and recent work from the daily log, linked pages
  capture "TEXT" | - | --file F | --gui  Drop a thought, link or file into raw/inbox/ (no session needed)
  health [--offline]            Check everything Kioku depends on and say how to fix problems
  check-file FILE...            Frontmatter, links and todo/event checks for just-written files
  today                         Create today's daily note if missing, print its path
  journal                       Create today's journal note (the user's own notes) if missing, print its path
  todo [--soon]                 Open todos in journal/, by due date; --soon: overdue and next 3 days
  agenda [--days N] [--past]    Upcoming events from calendar/ (default 7 days, recurrences expanded);
                                ended ones are left out, --past keeps today's
  remote on|off|status|pull|now Keep vault copies on several machines in step via git (pull/push)
  remote setup                  Install the vault's post-commit hook (push after each commit)
  update [--check]              Update Kioku (git pull + install step); --check only looks
  update --to vX | --rollback   Go to a release tag, or back to the version before the last update (pinned
                                until the next plain update)
  version                       Print the Kioku version and install fingerprint
  new <type> "<Title>"          Create a page: source|concept|entity|tool|idea|topic|synthesis|project|contact|review
  new event "<Title>" DATE      Create an event in calendar/YYYY/
  log <op> "<Title>"            Append to log.md: ingest|query|lint|note|setup
  search [--raw] [-n N] <term...>  Ranked search; each word is a term, a "quoted phrase" one term. Turkish-aware
  recap [--days N]              Daily, journal and log entries from the last N days (default 7)
  stale [--all]                 Wiki and project pages whose facts are due to be checked again (reviewed:)
  home [--print]                Write Home.md, the start page for Obsidian (also at session start and after commits)
  lint-check                    Broken links, orphans, uncatalogued or topic-less pages, frontmatter
  install [--vault PATH] …      Set up the vault and wire it into the agents (see install --help)
  hook <agent> <event>          Run a hook (used by the agents' settings; reads JSON on stdin)
"#;

fn main() {
    let code = match real_main() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("kioku: {e:#}");
            2
        }
    };
    std::process::exit(code);
}

fn real_main() -> Result<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    // Commands that work without a vault.
    match cmd {
        "" | "-h" | "--help" | "help" => {
            print!("{USAGE}");
            return Ok(0);
        }
        "install" => return install::run(rest),
        "hook" => return Ok(hook::run(rest)),
        "version" => {
            println!("{}", version::line(&util::sys_dir()));
            return Ok(0);
        }
        "whoami" => {
            let agent = match rest.first().map(String::as_str) {
                Some("--agent") => rest.get(1).cloned().unwrap_or_default(),
                _ => String::new(),
            };
            println!("{}", util::trace(&agent));
            return Ok(0);
        }
        _ => {}
    }
    let vault = util::vault();
    if !vault.is_dir() {
        return util::die(format!("vault not found: {}", vault.display()));
    }
    let path = |p: PathBuf| {
        println!("{}", p.display());
        Ok(0)
    };
    match cmd {
        "context" if rest.first().map(String::as_str) == Some("--deep") => {
            let q = rest[1..].join(" ");
            if q.trim().is_empty() {
                return util::die("usage: kioku context --deep <project or topic>");
            }
            println!("{}", deep::build(&vault, &q));
            Ok(0)
        }
        "context" => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| vault.clone());
            let agent = match rest.first().map(String::as_str) {
                Some("--agent") => rest.get(1).cloned().unwrap_or_default(),
                _ => String::new(),
            };
            println!("{}", context::build(&vault, &cwd, "", &agent));
            Ok(0)
        }
        "capture" => {
            if let Some(m) = notes::capture(&vault, rest)? {
                println!("{m}");
            }
            Ok(0)
        }
        "health" => health::run(&vault, rest),
        "check-file" => {
            let out = lint::check_file(&vault, rest);
            print!("{out}");
            Ok(if out.is_empty() { 0 } else { 1 })
        }
        "today" => path(notes::today(&vault)?),
        "journal" => path(notes::journal(&vault)?),
        "todo" => {
            print!("{}", notes::todo(&vault, rest.first().map(String::as_str) == Some("--soon")));
            Ok(0)
        }
        "agenda" => Ok(calendar::cmd_agenda(&vault, rest)),
        "remote" => git::cmd_remote(&vault, rest),
        "update" => update::run(&vault, rest),
        "new" => path(notes::new(&vault, rest)?),
        "log" => {
            println!("{}", notes::log(&vault, rest)?);
            Ok(0)
        }
        "search" => search::run(&vault, rest),
        "recap" => {
            print!("{}", notes::recap(&vault, rest)?);
            Ok(0)
        }
        "stale" => Ok(stale::run(&vault, rest)),
        "home" => home::run(&vault, rest),
        "lint-check" => {
            let (out, n) = lint::lint_check(&vault);
            print!("{out}");
            Ok(if n == 0 { 0 } else { 1 })
        }
        _ => {
            print!("{USAGE}");
            Ok(2)
        }
    }
}
