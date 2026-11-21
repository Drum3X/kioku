//! The dream pass: a weekly consolidation of the vault by an agent, on a branch of its own, so
//! nothing reaches the notes before the user says yes. `start` makes a copy of the vault (a git
//! worktree outside it, on branch `dream/YYYY-MM-DD`) and prints what to work on; the agent edits
//! the copy (kioku-dream skill); `finish` puts back anything it must not touch, drops the work
//! when search or the checks got worse, commits it on the branch and pushes the branch. The
//! session context then says a dream is waiting; `show`, `merge` (all or some files) and `drop`
//! act on it with the user's yes.
//!
//!   kioku dream start [--days N] | finish -m "summary" | status | show | merge [--only FILE…] | drop

use crate::{eval, lint, supersede, util};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Paths the dream pass must not change: the user's profile and rules, sources, the user's own
/// notes, reviews and the eval questions.
const KEEP: [&str; 10] = ["Core.md", "AGENTS.md", "CLAUDE.md", "raw/", "journal/", "calendar/", "contacts/", "reviews/", "eval/", ".kioku"];
/// Fewer work-log lines than this in the window: nothing worth a dream.
const MIN_LINES: usize = 5;
/// A drop of the eval MRR beyond this drops the dream.
const MAX_DROP: f64 = 0.02;

fn work_dir() -> PathBuf {
    util::state_dir().join("dream")
}
fn kept(p: &str) -> bool {
    KEEP.iter().any(|k| if k.ends_with('/') { p.starts_with(k) } else { p == *k || p.starts_with(&format!("{k}/")) })
}
fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let o = util::run(util::git(dir).args(args).env("KIOKU_GIT_SYNCING", "1").env("GIT_TERMINAL_PROMPT", "0"), 60)?;
    if !o.status.success() {
        anyhow::bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&o.stdout).trim_end().to_string())
}
fn has_origin(vault: &Path) -> bool {
    util::git_out(vault, &["remote", "get-url", "origin"]).is_some()
}

/// Dream branches not merged yet (local or on the remote), oldest first, by name.
pub fn pending(vault: &Path) -> Vec<String> {
    let refs = util::git_out(vault, &["for-each-ref", "--format=%(refname:short)", "refs/heads/dream", "refs/remotes/origin/dream"]).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    for r in refs.lines().filter(|r| !r.is_empty()) {
        let name = r.strip_prefix("origin/").unwrap_or(r).to_string();
        let merged = util::run(util::git(vault).args(["merge-base", "--is-ancestor", r, "HEAD"]), 10).is_ok_and(|o| o.status.success());
        if !merged && !out.contains(&name) {
            out.push(name);
        }
    }
    out.sort();
    out
}
/// The ref to read a dream branch from: the local branch, else the remote one.
fn reference(vault: &Path, name: &str) -> String {
    if util::git_out(vault, &["rev-parse", "--verify", "-q", &format!("refs/heads/{name}")]).is_some() {
        name.to_string()
    } else {
        format!("origin/{name}")
    }
}
fn pick(vault: &Path, arg: Option<&String>) -> Result<String> {
    let p = pending(vault);
    match arg {
        Some(a) if p.contains(a) => Ok(a.clone()),
        Some(a) => util::die(format!("no dream branch {a} waiting (kioku dream status)")),
        None => match p.last() {
            Some(b) => Ok(b.clone()),
            None => util::die("no dream waiting"),
        },
    }
}
/// The session context's line about a waiting dream, if any.
pub fn context_line(vault: &Path) -> Option<String> {
    let b = pending(vault).pop()?;
    let r = reference(vault, &b);
    let n = util::git_out(vault, &["diff", "--name-only", &format!("HEAD...{r}")]).map(|s| s.lines().count()).unwrap_or(0);
    let v = vault.display();
    Some(format!(
        "A dream (the weekly consolidation) waits on branch {b}: {n} file(s). Tell the user in one line at a natural moment and offer to go through it: `{v}/.kioku/bin/kioku dream show`, then summarize the changes in a few plain bullets. With their yes: `kioku dream merge` (or `--only FILE…` for some), or `kioku dream drop`."
    ))
}

fn drop_branch(vault: &Path, b: &str) {
    let _ = git(vault, &["branch", "-D", b]);
    if has_origin(vault) {
        let _ = git(vault, &["push", "-q", "origin", "--delete", b]);
    }
}
fn remove_work(vault: &Path) {
    let d = work_dir();
    let _ = git(vault, &["worktree", "remove", "--force", &d.to_string_lossy()]);
    let _ = fs::remove_dir_all(&d);
    let _ = git(vault, &["worktree", "prune"]);
}

fn start(vault: &Path, args: &[String]) -> Result<i32> {
    let days: i64 = args.iter().position(|a| a == "--days").and_then(|i| args.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(7);
    let to = util::today();
    let from = to - chrono::Duration::days(days - 1);
    let (f, t) = (from.format("%Y-%m-%d").to_string(), to.format("%Y-%m-%d").to_string());
    let lines: usize = util::md_files(&vault.join("daily"))
        .iter()
        .filter(|p| p.file_stem().is_some_and(|s| (f.as_str()..=t.as_str()).contains(&s.to_string_lossy().as_ref())))
        .map(|p| util::read(p).lines().filter(|l| l.starts_with("- ") && l.contains("] ")).count())
        .sum();
    if lines < MIN_LINES {
        println!("SKIP: only {lines} work-log line(s) from {f} to {t}; nothing to consolidate.");
        return Ok(0);
    }
    let _ = crate::git::sync(vault, "pull");
    // Local dream branches merged elsewhere are done.
    for r in util::git_out(vault, &["branch", "--merged", "HEAD", "--format=%(refname:short)", "--list", "dream/*"]).unwrap_or_default().lines() {
        let _ = git(vault, &["branch", "-d", r]);
    }
    // A newer dream replaces one that is still waiting.
    for b in pending(vault) {
        drop_branch(vault, &b);
        println!("(dropped the earlier dream {b}, not merged)");
    }
    remove_work(vault);
    let b = format!("dream/{t}");
    let _ = git(vault, &["branch", "-D", &b]);
    let d = work_dir();
    fs::create_dir_all(d.parent().unwrap())?;
    git(vault, &["worktree", "add", "-q", "-b", &b, &d.to_string_lossy(), "HEAD"])?;
    let sys = util::canon(&vault.join(".kioku"));
    if sys.is_dir() {
        let _ = crate::install::symlink(&sys, &d.join(".kioku"));
    }
    let (lint_out, _) = lint::lint_check(&d);
    let sup: String = supersede::candidates(&d, days.max(30), crate::semantic::ready())
        .iter()
        .map(|(a, b)| format!("- older: {}\n  newer: {}\n", supersede::line(a), supersede::line(b)))
        .collect();
    println!("DREAM {}", util::slash(&d));
    println!("Branch {b}; window: daily/{f} … daily/{t} ({lines} lines).");
    println!("Work only in that folder (a copy of the vault; the vault itself stays as it is) and follow the kioku-dream skill. Then run: kioku dream finish -m \"<one line per change>\"\n");
    println!("## Check report: kioku lint-check\n{lint_out}");
    println!("## Check report: kioku supersedes\n{}", if sup.is_empty() { "(none)\n".into() } else { sup });
    Ok(0)
}

fn finish(vault: &Path, args: &[String]) -> Result<i32> {
    let d = work_dir();
    if !d.join(".git").exists() {
        return util::die("no dream in progress (kioku dream start)");
    }
    let msg = args.iter().position(|a| a == "-m").and_then(|i| args.get(i + 1)).cloned().unwrap_or_default();
    let b = git(&d, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    // Put back what the dream must not change, and anything deleted.
    let mut put_back = Vec::new();
    for l in git(&d, &["status", "--porcelain", "--untracked-files=all"])?.lines() {
        let (st, p) = (&l[..2], l[3..].trim_matches('"').to_string());
        if kept(&p) || st.contains('D') {
            if st == "??" {
                let _ = fs::remove_file(d.join(&p));
            } else {
                git(&d, &["checkout", "HEAD", "--", &p])?;
            }
            put_back.push(p);
        }
    }
    if !put_back.is_empty() {
        println!("Put back (the dream may not change or delete these): {}", put_back.join(", "));
    }
    let changed: Vec<String> = git(&d, &["status", "--porcelain", "--untracked-files=all"])?.lines().map(|l| l[3..].trim_matches('"').to_string()).collect();
    let end = |why: &str| -> Result<i32> {
        remove_work(vault);
        let _ = git(vault, &["branch", "-D", &b]);
        println!("{why}");
        Ok(0)
    };
    if changed.is_empty() {
        return end("No changes: the dream found nothing to consolidate.");
    }
    let secrets: String = changed.iter().map(|p| crate::secrets::report(p, &util::read(&d.join(p)))).collect();
    if !secrets.is_empty() {
        return end(&format!("Dropped: the changes contain what looks like a secret.\n{secrets}"));
    }
    // Search must not get worse, nor the checks.
    std::env::set_var("KIOKU_SEMANTIC", "off");
    let before = eval::mrr(vault, vault);
    let after = eval::mrr(&d, vault);
    let scores = match (before, after) {
        (Some(x), Some(y)) => {
            let s = format!("search MRR by terms {:.2} → {:.2}, by question {:.2} → {:.2}", x.0, y.0, x.1, y.1);
            if y.0 < x.0 - MAX_DROP || y.1 < x.1 - MAX_DROP {
                return end(&format!("Dropped: search got worse ({s})."));
            }
            s
        }
        _ => "search not measured (no eval questions)".into(),
    };
    let (i0, i1) = (lint::lint_check(vault).1, lint::lint_check(&d).1);
    if i1 > i0 {
        return end(&format!("Dropped: lint-check issues went from {i0} to {i1}."));
    }
    git(&d, &["add", "-A"])?;
    let none = util::state_dir().join("dream-no-hooks");
    fs::create_dir_all(&none)?;
    let body = format!("{b}\n\n{}\n\n{scores}; lint-check issues {i0} → {i1}\n", msg.trim());
    git(&d, &["-c", &format!("core.hooksPath={}", util::slash(&none)), "commit", "-q", "-m", &body])?;
    let stat = git(&d, &["diff", "--stat", "HEAD~1", "HEAD"])?;
    remove_work(vault);
    let pushed = if has_origin(vault) {
        match git(vault, &["push", "-q", "-u", "origin", &b]) {
            Ok(_) => " (pushed)".to_string(),
            Err(e) => format!(" (not pushed: {e})"),
        }
    } else {
        String::new()
    };
    println!("Dream ready on branch {b}{pushed}: {} file(s); {scores}; lint-check issues {i0} → {i1}.\n{stat}", changed.len());
    println!("The user decides in a later session (the session context says it waits): kioku dream show / merge / drop.");
    Ok(0)
}

fn show(vault: &Path, args: &[String]) -> Result<i32> {
    let b = pick(vault, args.first().filter(|a| !a.starts_with('-')))?;
    let r = reference(vault, &b);
    println!("{}\n", git(vault, &["log", "-1", "--format=%B", &r])?.trim_end());
    println!("{}", git(vault, &["diff", "--stat", &format!("HEAD...{r}")])?);
    println!("\n{}", git(vault, &["diff", &format!("HEAD...{r}")])?);
    Ok(0)
}

fn merge(vault: &Path, args: &[String]) -> Result<i32> {
    let only: Vec<&String> = args.iter().skip_while(|a| *a != "--only").skip(1).collect();
    let b = pick(vault, args.first().filter(|a| !a.starts_with('-')))?;
    let r = reference(vault, &b);
    let _ = crate::git::sync(vault, "pull");
    if only.is_empty() {
        let o = util::run(util::git(vault).args(["merge", "--no-ff", "-q", "-m", &format!("dream: merge {b}"), &r]), 60)?;
        if !o.status.success() {
            let _ = git(vault, &["merge", "--abort"]);
            return util::die(format!("{b} conflicts with newer changes in the vault: merge some files with --only, or drop it"));
        }
    } else {
        let mut a = vec!["checkout", r.as_str(), "--"];
        a.extend(only.iter().map(|s| s.as_str()));
        git(vault, &a)?;
        let o = util::run(util::git(vault).args(["commit", "-q", "-m", &format!("dream: merge part of {b}")]), 60)?;
        if !o.status.success() {
            return util::die(format!("nothing to merge from {b} for those files"));
        }
    }
    drop_branch(vault, &b);
    let _ = crate::git::sync(vault, "now");
    println!("merged {b}{}", if only.is_empty() { String::new() } else { format!(" ({} file(s))", only.len()) });
    Ok(0)
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str).unwrap_or("status") {
        "start" => start(vault, rest),
        "finish" => finish(vault, rest),
        "show" => show(vault, rest),
        "merge" => merge(vault, rest),
        "drop" => {
            let b = pick(vault, rest.first())?;
            drop_branch(vault, &b);
            println!("dropped {b}");
            Ok(0)
        }
        "status" => {
            let p = pending(vault);
            if p.is_empty() {
                println!("No dream waiting.");
            }
            for b in p {
                let r = reference(vault, &b);
                println!("{b}: {}", util::git_out(vault, &["diff", "--shortstat", &format!("HEAD...{r}")]).unwrap_or_default().trim());
            }
            Ok(0)
        }
        _ => util::die("usage: kioku dream start [--days N] | finish -m \"summary\" | status | show | merge [--only FILE…] | drop"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kept_paths() {
        assert!(kept("Core.md") && kept("journal/2026-11-21.md") && kept(".kioku") && kept("eval/x.tsv"));
        assert!(!kept("daily/2026-11-21.md") && !kept("wiki/concepts/x.md") && !kept("Core.md.bak"));
    }
}
