//! `kioku home [--print]`: Home.md at the vault root, a start page for browsing the vault in
//! Obsidian: what is coming up, due todos, active projects, open threads, recent changes,
//! pages due for review and the inbox. It is generated (session start, after each commit,
//! or this command), kept out of git (.gitignore) and never edited by hand.

use crate::{calendar, fm, handoff, notes, stale, util};
use anyhow::Result;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub const FILE: &str = "Home.md";

fn link(rel: &str, label: &str) -> String {
    let target = rel.trim_end_matches(".md");
    if label.is_empty() {
        format!("[[{target}]]")
    } else {
        format!("[[{target}|{label}]]")
    }
}
fn title(text: &str, rel: &str) -> String {
    let t = fm::field(text, "title");
    let t = t.trim().trim_matches(|c| c == '"' || c == '\'');
    if t.is_empty() {
        rel.rsplit('/').next().unwrap_or(rel).trim_end_matches(".md").to_string()
    } else {
        t.to_string()
    }
}
/// "… — path/to/x.md[:line]" → "… ([[path/to/x|↗]])"
fn relink(line: &str) -> String {
    match line.rsplit_once(" — ") {
        Some((head, rel)) => {
            let rel = rel.split(':').next().unwrap_or(rel);
            format!("{head} ({})", link(rel, "↗"))
        }
        None => line.to_string(),
    }
}

pub fn render(vault: &Path) -> String {
    let now = chrono::Local::now();
    let mut o = String::new();
    let mut add = |s: &str| {
        o.push_str(s);
        o.push('\n');
    };
    add("# Home");
    add("");
    add("<!-- kioku:generated — written by `kioku home` at session start and after each commit; edits are overwritten. Not in git. -->");
    add(&format!("*Updated {} on {}.*", now.format("%Y-%m-%d %H:%M"), util::host()));
    add("");

    add("## Coming up");
    let ag = calendar::agenda(vault, 7, false);
    for l in ag.lines() {
        add(&if l.starts_with("- ") { relink(l) } else { format!("- {l}") });
    }
    add("");

    let hs = handoff::open(vault);
    if !hs.is_empty() {
        add("## Handoffs (where a session stopped)");
        for h in hs.iter().take(8) {
            add(&format!("- {} ({})", h.line(300), link(&format!("daily/{}", h.date), "↗")));
        }
        add("");
    }

    add("## Todos");
    let td = notes::todo(vault, true);
    for l in td.lines() {
        add(&if l.starts_with("- … ") || !l.starts_with("- ") {
            l.to_string()
        } else {
            relink(l)
        });
    }
    add("");

    let projects = util::md_files(&vault.join("projects"));
    let mut act = Vec::new();
    for f in &projects {
        let t = util::read(f);
        if fm::field(&t, "status") != "active" {
            continue;
        }
        let rel = util::rel(vault, f);
        let next = fm::field(&t, "next");
        let hosts = fm::list(&t, "hosts");
        let on = if hosts.is_empty() { String::new() } else { format!(" ({})", hosts.join(", ")) };
        act.push(format!(
            "- {}{on}{}",
            link(&rel, &title(&t, &rel)),
            if next.is_empty() {
                String::new()
            } else {
                format!(" — next: {next}")
            }
        ));
    }
    if !act.is_empty() {
        add("## Active projects");
        for a in &act {
            add(a);
        }
        add("");
    }

    // Open threads from the last 30 days of the work log, newest first.
    static OPEN: OnceLock<Regex> = OnceLock::new();
    let re = OPEN.get_or_init(|| Regex::new(r"^- [0-9:]+ \[([^\]]*)\] open: (.*)$").unwrap());
    let since = util::ymd(util::day(-30));
    let mut open = Vec::new();
    for f in util::md_files(&vault.join("daily")).into_iter().rev() {
        let d = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        if d < since {
            continue;
        }
        let t = util::read(&f);
        if fm::is_private(&t) {
            continue;
        }
        for l in t.lines().rev() {
            if let Some(c) = re.captures(l) {
                let tag = c[1].split('·').next().unwrap_or("").trim().to_string();
                open.push(format!(
                    "- {d} [{tag}] {} ({})",
                    util::take(&c[2], 220),
                    link(&format!("daily/{d}"), "↗")
                ));
            }
        }
    }
    if !open.is_empty() {
        add("## Open threads (last 30 days)");
        for l in open.iter().take(12) {
            add(l);
        }
        if open.len() > 12 {
            add(&format!("- … {} more in daily/", open.len() - 12));
        }
        add("");
    }

    // Recently changed wiki and project pages (by updated:), last 7 days.
    let week = util::ymd(util::day(-7));
    let mut recent: Vec<(String, String)> = Vec::new();
    for dir in ["wiki", "projects", "reviews"] {
        for f in util::md_files(&vault.join(dir)) {
            let t = util::read(&f);
            let up = fm::field(&t, "updated");
            let up = up.trim();
            if up.len() >= 10 && &up[..10] >= week.as_str() {
                let rel = util::rel(vault, &f);
                recent.push((up[..10].to_string(), format!("- {} — {}", link(&rel, &title(&t, &rel)), &up[..10])));
            }
        }
    }
    recent.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    if !recent.is_empty() {
        add("## Recently changed");
        for (_, l) in recent.iter().take(15) {
            add(l);
        }
        if recent.len() > 15 {
            add(&format!("- … {} more", recent.len() - 15));
        }
        add("");
    }

    let due = stale::pages(vault, false);
    if !due.is_empty() {
        add("## Due for review (are these still true?)");
        for d in due.iter().take(10) {
            let last = d.last.map(util::ymd).unwrap_or_else(|| "never".into());
            add(&format!("- {} — last checked {last}", link(&d.rel, &d.title)));
        }
        add("");
    }

    let mut inbox: Vec<String> = walkdir::WalkDir::new(vault.join("raw/inbox"))
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.file_name() != ".gitkeep")
        .map(|e| util::rel(vault, e.path()))
        .collect();
    inbox.sort();
    if !inbox.is_empty() {
        add(&format!("## Inbox ({} waiting; say \"process the inbox\")", inbox.len()));
        for r in inbox.iter().take(10) {
            add(&format!("- {}", link(r, r.rsplit('/').next().unwrap_or(r))));
        }
        add("");
    }

    let cnt = |d: &str| util::md_files(&vault.join("wiki").join(d)).len();
    add("## Brain");
    add(&format!(
        "{} · {} topics · {} concepts · {} entities · {} ideas · {} sources · {} syntheses",
        link("index", "Index"),
        cnt("topics"),
        cnt("concepts"),
        cnt("entities"),
        cnt("ideas"),
        cnt("sources"),
        cnt("synthesis")
    ));
    o
}

/// Write Home.md when its content changed (the timestamp line aside). Ok(true) when written.
pub fn write(vault: &Path) -> Result<bool> {
    let f = vault.join(FILE);
    let new = render(vault);
    let strip = |s: &str| s.lines().filter(|l| !l.starts_with("*Updated ")).collect::<Vec<_>>().join("\n");
    if f.exists() && strip(&util::read(&f)) == strip(&new) {
        return Ok(false);
    }
    util::write(&f, &new)?;
    Ok(true)
}

/// The automatic refresh (session start, after commits and pulls): only once the vault's
/// .gitignore keeps Home.md out of git, so it never shows up as an uncommitted change.
pub fn refresh(vault: &Path) {
    if util::read(&vault.join(".gitignore")).lines().any(|l| l.trim() == FILE) {
        let _ = write(vault);
    }
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    if args.first().map(String::as_str) == Some("--auto") {
        refresh(vault);
        return Ok(0);
    }
    if args.first().map(String::as_str) == Some("--print") {
        print!("{}", render(vault));
        return Ok(0);
    }
    write(vault)?;
    println!("{}", vault.join(FILE).display());
    Ok(0)
}
