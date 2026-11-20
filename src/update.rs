//! `kioku update [--check | --to vX | --rollback]`: move the checkout the vault links to, then
//! run the install step again with the options it was installed with (through that version's
//! install.sh, which also fetches or builds the matching binary). Copies from the Nix store are
//! updated through the flake.
//!
//!   kioku update              the latest version (the default branch); ends a pin
//!   kioku update --check      only look for new commits
//!   kioku update --to v0.2.1  that release tag, pinned until the next plain update
//!   kioku update --rollback   back to the version before the last update (or the previous tag)
//!
//! What it moved from, and a pin, are kept in ~/.config/kioku/update (the install record is
//! rewritten by every install).

use crate::util::{self, die};
use crate::version::VERSION;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

fn state_file() -> PathBuf {
    util::config_dir().join("update")
}
pub fn state(key: &str) -> Option<String> {
    let s = util::read(&state_file());
    s.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(str::to_string))
        .filter(|v| !v.is_empty())
}
fn set_state(previous: &str, pinned: Option<&str>) -> Result<()> {
    util::write(&state_file(), &format!("previous={previous}\npinned={}\n", pinned.unwrap_or("")))
}

/// The version of a checkout at `rev` (its VERSION file), "" if unknown.
fn version_at(sys: &Path, rev: &str) -> String {
    util::git_out(sys, &["show", &format!("{rev}:VERSION")])
        .map(|v| v.trim().to_string())
        .unwrap_or_default()
}
/// Release tags, newest first.
fn tags(sys: &Path) -> Vec<String> {
    util::git_out(sys, &["tag", "--list", "v*", "--sort=-v:refname"])
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}
/// A readable name for a commit: its release tag, else the commit id.
fn name_of(sys: &Path, rev: &str) -> String {
    util::git_out(sys, &["describe", "--tags", "--exact-match", rev]).unwrap_or_else(|| rev.to_string())
}

/// The checkout's default branch (origin/HEAD), else main.
fn default_branch(sys: &Path) -> String {
    util::git_out(sys, &["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
        .and_then(|r| r.strip_prefix("origin/").map(str::to_string))
        .unwrap_or_else(|| "main".into())
}

/// That version's bootstrap: install.sh, or install.ps1 through PowerShell on Windows.
fn install(sys: &Path, vault: &Path) -> Result<i32> {
    let flags = util::record_get("flags").unwrap_or_default();
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"]).arg(sys.join("install.ps1"));
        c
    } else {
        Command::new(sys.join("install.sh"))
    };
    let st = cmd
        .arg("--vault")
        .arg(vault)
        .args(flags.split_whitespace())
        .status()?;
    Ok(st.code().unwrap_or(1))
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let usage = "usage: kioku update [--check | --to vX.Y.Z | --rollback]";
    let mode = args.first().map(String::as_str).unwrap_or("");
    let target = match mode {
        "" | "--check" | "--rollback" => None,
        "--to" => Some(args.get(1).cloned().ok_or_else(|| anyhow::anyhow!("{usage}"))?),
        _ => return die(usage),
    };
    let sys = util::sys_dir();
    if sys.starts_with("/nix/store") {
        if mode == "--to" || mode == "--rollback" {
            println!(
                "Kioku {VERSION} comes from the Nix store, pinned by your flake. To go back, in your config repo run: nix flake lock --override-input kioku github:Drum3X/kioku/<tag> (or git checkout the previous flake.lock), then rebuild."
            );
        } else {
            println!("Kioku {VERSION} comes from the Nix store. Update it with: nix flake update kioku, then rebuild.");
        }
        return Ok(0);
    }
    if !sys.join(".git").exists() {
        return die(format!(
            "{} is not a git checkout; update it the way you installed it",
            sys.display()
        ));
    }
    let g = |a: &[&str]| util::run(util::git(&sys).args(a), 60).is_ok_and(|o| o.status.success());
    let head = util::git_out(&sys, &["rev-parse", "HEAD"]).unwrap_or_default();
    let pinned = state("pinned");
    // The version this checkout is at (the running binary may be another one).
    let cur = Some(version_at(&sys, "HEAD"))
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| VERSION.to_string());

    if mode == "--check" {
        if !g(&["fetch", "-q", "--tags"]) {
            return die(format!("git fetch failed in {}", sys.display()));
        }
        if let Some(p) = &pinned {
            println!("Kioku {cur} is pinned to {p} (kioku update returns to the latest)");
        }
        let branch = default_branch(&sys);
        let up = format!("HEAD..origin/{branch}");
        let n: usize = util::git_out(&sys, &["rev-list", "--count", &up])
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            println!("Kioku {cur}: {n} new commit(s) available; run: kioku update");
            for l in util::git_out(&sys, &["log", "--oneline", &up]).unwrap_or_default().lines().take(20) {
                println!("{l}");
            }
        } else {
            println!("Kioku {cur} is up to date");
        }
        return Ok(0);
    }

    // Moving the checkout: refuse with local changes (bin/ and target/ are ignored).
    if !util::git_out(&sys, &["status", "--porcelain", "--untracked-files=no"])
        .unwrap_or_default()
        .is_empty()
    {
        return die(format!(
            "{} has uncommitted changes; commit or stash them, then run kioku update again",
            sys.display()
        ));
    }
    if mode == "--to" || mode == "--rollback" {
        if !g(&["fetch", "-q", "--tags"]) {
            eprintln!("kioku: git fetch failed; using the tags already here");
        }
        let all = tags(&sys);
        let to = match (&target, mode) {
            (Some(t), _) => {
                let t = if t.starts_with('v') { t.clone() } else { format!("v{t}") };
                if !all.contains(&t) {
                    return die(format!(
                        "no release tag {t}; there are: {}",
                        all.iter().take(10).cloned().collect::<Vec<_>>().join(" ")
                    ));
                }
                t
            }
            _ => {
                // The version before the last update, else the newest tag older than this one.
                let prev = state("previous")
                    .filter(|p| util::git_out(&sys, &["rev-parse", &format!("{p}^{{commit}}")]).as_deref() != Some(head.as_str()));
                match prev {
                    Some(p) => p,
                    None => match all.iter().find(|t| {
                        version_at(&sys, t) != VERSION && util::git_out(&sys, &["merge-base", "--is-ancestor", t, "HEAD"]).is_some()
                    }) {
                        Some(t) => t.clone(),
                        None => return die("no earlier version to go back to"),
                    },
                }
            }
        };
        if !g(&["checkout", "-q", "--detach", &to]) {
            return die(format!("git checkout {to} failed in {}", sys.display()));
        }
        let v = version_at(&sys, "HEAD");
        set_state(&name_of(&sys, &head), Some(&to))?;
        println!(
            "Kioku moved from {cur} to {} ({to}); pinned until you run kioku update.",
            if v.is_empty() { "?" } else { &v }
        );
        return install(&sys, vault);
    }

    // The latest version: back on the default branch (ends a pin), then pull.
    let branch = default_branch(&sys);
    let on_branch = util::git_out(&sys, &["symbolic-ref", "-q", "--short", "HEAD"]).as_deref() == Some(branch.as_str());
    if !on_branch && !g(&["checkout", "-q", &branch]) {
        return die(format!("git checkout {branch} failed in {}", sys.display()));
    }
    if !g(&["pull", "--ff-only", "-q"]) {
        return die(format!(
            "git pull failed in {} (local changes or a diverged branch?); fix that, then run kioku update again",
            sys.display()
        ));
    }
    let after = util::git_out(&sys, &["rev-parse", "HEAD"]).unwrap_or_default();
    if pinned.is_some() || head != after {
        set_state(&name_of(&sys, &head), None)?;
    }
    if head == after {
        println!("Kioku {cur}: no new commits");
    } else {
        println!("Kioku updated to {}:", version_at(&sys, "HEAD"));
        for l in util::git_out(&sys, &["log", "--oneline", &format!("{head}..{after}")])
            .unwrap_or_default()
            .lines()
            .take(20)
        {
            println!("  {l}");
        }
    }
    if let Some(p) = &pinned {
        println!("The pin to {p} ended; following the latest version again.");
    }
    install(&sys, vault)
}
