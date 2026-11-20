//! Several copies of the vault kept in step through its git remote (kioku remote …), and the
//! vault's post-commit hook.

use crate::util::{self, die};
use anyhow::Result;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

const HOOK: &str = r#"#!/bin/sh
# kioku: after each commit, refresh Home.md and push the vault to its git remote (kioku remote on)
B="$(git rev-parse --show-toplevel)/.kioku/bin/kioku"
[ -f "$B.exe" ] && B="$B.exe"
[ -f "$B" ] && [ -z "${KIOKU_GIT_SYNCING:-}" ] || exit 0
# Refresh Home.md (the Obsidian start page; not in git), then push with autosync. kioku bg runs
# them detached (its own process group / a detached process on Windows), so they survive an
# agent that kills its command's process group as soon as the commit returns.
( unset GIT_DIR GIT_INDEX_FILE GIT_WORK_TREE; "$B" bg home --auto )
[ "$(git config --bool kioku.autosync)" = true ] || exit 0
( unset GIT_DIR GIT_INDEX_FILE GIT_WORK_TREE; "$B" bg remote now )
exit 0
"#;

/// `kioku bg <args…>`: run this program with the args detached, and return at once.
pub fn bg(vault: &Path, args: &[String]) -> Result<i32> {
    use std::process::{Command, Stdio};
    let exe = std::env::current_exe()?;
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .env("KIOKU_DIR", vault)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn()?;
    Ok(0)
}

fn ours(text: &str) -> bool {
    text.lines().any(|l| l.starts_with("# kioku:") || l.starts_with("# second-brain:"))
}

/// Install the post-commit hook; remove the pre-commit hook of earlier versions (CalDAV ids).
/// Returns what changed, one line each.
pub fn setup_hooks(vault: &Path) -> Result<Vec<String>> {
    if !vault.join(".git").is_dir() {
        return die("the vault is not a git repository");
    }
    let hd = vault.join(".git/hooks");
    fs::create_dir_all(&hd)?;
    let mut msgs = Vec::new();
    let f = hd.join("post-commit");
    if f.exists() && !ours(&util::read(&f)) {
        eprintln!("kioku: {} exists and is not ours; left untouched", f.display());
    } else if util::read(&f) != HOOK {
        fs::write(&f, HOOK)?;
        set_exec(&f)?;
        msgs.push("installed post-commit hook".to_string());
    }
    let pre = hd.join("pre-commit");
    if pre.exists() && ours(&util::read(&pre)) {
        fs::remove_file(&pre)?;
        msgs.push("removed the old pre-commit hook".to_string());
    }
    Ok(msgs)
}
#[cfg(unix)]
pub fn set_exec(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = fs::metadata(p)?.permissions();
    perm.set_mode(perm.mode() | 0o755);
    fs::set_permissions(p, perm)?;
    Ok(())
}
#[cfg(not(unix))]
pub fn set_exec(_: &Path) -> Result<()> {
    Ok(())
}

fn conflict_file() -> std::path::PathBuf {
    util::state_dir().join("git-sync.conflict")
}
fn log_line(msg: &str) {
    use std::io::Write;
    let _ = fs::create_dir_all(util::state_dir());
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(util::state_dir().join("git-sync.log"))
    {
        let _ = writeln!(f, "{} {msg}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// Pull (rebase, autostash) and, for "now", push. Does nothing when autosync is off. Ok(false)
/// on a failure that was logged.
pub fn sync(vault: &Path, mode: &str) -> Result<bool> {
    if !util::autosync(vault) {
        return Ok(true);
    }
    let st = util::state_dir();
    fs::create_dir_all(&st)?;
    let lock = st.join("git-sync.lock");
    // A stale lock from a killed run (older than 2 minutes).
    if let Ok(md) = fs::metadata(&lock) {
        if md
            .modified()
            .ok()
            .and_then(|m| SystemTime::now().duration_since(m).ok())
            .is_some_and(|d| d > Duration::from_secs(120))
        {
            let _ = fs::remove_dir(&lock);
        }
    }
    let mut got = false;
    for _ in 0..10 {
        if fs::create_dir(&lock).is_ok() {
            got = true;
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    if !got {
        eprintln!("kioku: git sync busy ({})", lock.display());
        return Ok(false);
    }
    let res = sync_locked(vault, mode);
    let _ = fs::remove_dir(&lock);
    res
}

fn sync_locked(vault: &Path, mode: &str) -> Result<bool> {
    let g = |args: &[&str]| {
        let mut c = util::git(vault);
        c.args(args).env("KIOKU_GIT_SYNCING", "1").env("GIT_TERMINAL_PROMPT", "0");
        c
    };
    if vault.join(".git/rebase-merge").is_dir() || vault.join(".git/rebase-apply").is_dir() {
        fs::write(
            conflict_file(),
            "a rebase is in progress in the vault; resolve it, then run kioku remote now\n",
        )?;
        return Ok(false);
    }
    let mut last = String::new();
    for attempt in 1..=3u64 {
        let o = util::run(&mut g(&["pull", "--rebase", "--autostash", "-q"]), 20)?;
        if !o.status.success() {
            let _ = util::run(&mut g(&["rebase", "--abort"]), 10);
            let out = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
            log_line(&format!("pull failed: {}", out.trim_end()));
            if out.to_lowercase().contains("conflict") {
                fs::write(
                    conflict_file(),
                    format!(
                        "pull conflicted with the remote ({}); local commits kept, nothing pushed\n",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
                    ),
                )?;
            }
            return Ok(false);
        }
        let _ = fs::remove_file(conflict_file());
        crate::home::refresh(vault);
        // Local commits not pushed yet (a push after a commit that got cut off) go out with
        // any pull too, so nothing stays behind on one machine.
        let ahead = util::git_out(vault, &["log", "@{u}..HEAD", "--oneline"]).unwrap_or_default();
        if ahead.is_empty() {
            return Ok(true);
        }
        let _ = mode;
        let o = util::run(&mut g(&["push", "-q"]), 30)?;
        if o.status.success() {
            return Ok(true);
        }
        last = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        let l = last.to_lowercase();
        if !(l.contains("rejected") || l.contains("fetch first") || l.contains("non-fast-forward")) {
            break;
        }
        std::thread::sleep(Duration::from_secs(attempt));
    }
    log_line(&format!("push failed: {}", last.trim_end()));
    Ok(false)
}

pub fn cmd_remote(vault: &Path, args: &[String]) -> Result<i32> {
    match args.first().map(String::as_str).unwrap_or("status") {
        "on" => {
            if util::git_out(vault, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).is_none() {
                return die(format!(
                    "the vault branch has no upstream; push it once with: git -C {} push -u origin HEAD",
                    vault.display()
                ));
            }
            util::git_out(vault, &["config", "kioku.autosync", "true"]);
            setup_hooks(vault)?;
            println!("autosync on: pull at session start and before prompts, push after each commit");
        }
        "off" => {
            let _ = util::run(util::git(vault).args(["config", "--unset", "kioku.autosync"]), 0);
            println!("autosync off");
        }
        "status" => {
            println!("autosync: {}", if util::autosync(vault) { "on" } else { "off" });
            if conflict_file().exists() {
                println!("conflict: {}", util::read(&conflict_file()).trim_end());
            }
            if let Some(s) = util::git_out(vault, &["status", "-sb"]) {
                println!("{}", s.lines().next().unwrap_or(""));
            }
        }
        m @ ("pull" | "now") => return Ok(if sync(vault, m)? { 0 } else { 1 }),
        "setup" => {
            for m in setup_hooks(vault)? {
                println!("{m}");
            }
        }
        _ => return die("usage: kioku remote on|off|status|pull|now|setup"),
    }
    Ok(0)
}
