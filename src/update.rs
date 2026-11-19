//! `kioku update [--check]`: pull the checkout the vault links to, then run the install step
//! again with the options it was installed with (through install.sh, which also fetches or builds
//! the matching binary). Copies from the Nix store are updated through the flake.

use crate::util::{self, die};
use crate::version::VERSION;
use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let check = match args.first().map(String::as_str) {
        Some("--check") => true,
        None => false,
        _ => return die("usage: kioku update [--check]"),
    };
    let sys = util::sys_dir();
    if sys.starts_with("/nix/store") {
        println!("Kioku {VERSION} comes from the Nix store. Update it with: nix flake update kioku, then rebuild.");
        return Ok(0);
    }
    if !sys.join(".git").exists() {
        return die(format!(
            "{} is not a git checkout; update it the way you installed it",
            sys.display()
        ));
    }
    let g = |a: &[&str]| util::run(util::git(&sys).args(a), 60);
    if check {
        if !g(&["fetch", "-q"]).is_ok_and(|o| o.status.success()) {
            return die(format!("git fetch failed in {}", sys.display()));
        }
        let n: usize = util::git_out(&sys, &["rev-list", "--count", "HEAD..@{u}"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        if n > 0 {
            println!("Kioku {VERSION}: {n} new commit(s) available; run: kioku update");
            for l in util::git_out(&sys, &["log", "--oneline", "HEAD..@{u}"])
                .unwrap_or_default()
                .lines()
                .take(20)
            {
                println!("{l}");
            }
        } else {
            println!("Kioku {VERSION} is up to date");
        }
        return Ok(0);
    }
    let before = util::git_out(&sys, &["rev-parse", "HEAD"]).unwrap_or_default();
    if !g(&["pull", "--ff-only", "-q"]).is_ok_and(|o| o.status.success()) {
        return die(format!(
            "git pull failed in {} (local changes or a diverged branch?); fix that, then run kioku update again",
            sys.display()
        ));
    }
    let after = util::git_out(&sys, &["rev-parse", "HEAD"]).unwrap_or_default();
    if before == after {
        println!("Kioku {VERSION}: no new commits");
    } else {
        println!("Kioku updated:");
        for l in util::git_out(&sys, &["log", "--oneline", &format!("{before}..{after}")])
            .unwrap_or_default()
            .lines()
            .take(20)
        {
            println!("  {l}");
        }
    }
    let flags = util::record_get("flags").unwrap_or_default();
    let mut cmd = Command::new(sys.join("install.sh"));
    cmd.arg("--vault").arg(vault).args(flags.split_whitespace());
    let st = cmd.status()?;
    Ok(st.code().unwrap_or(1))
}
