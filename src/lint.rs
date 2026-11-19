//! Vault checks: lint-check (the whole vault) and check-file (just-written files).

use crate::{calendar, fm, stale, util};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Every vault file (relative path), plus lookups without extension and by basename.
pub struct Index {
    exists: HashSet<String>,
    byname: HashMap<String, String>,
}
impl Index {
    pub fn build(vault: &Path) -> Index {
        let mut exists = HashSet::new();
        let mut byname = HashMap::new();
        let prune = [".git", ".kioku", ".obsidian", ".trash", ".claude"];
        let mut files: Vec<String> = walkdir::WalkDir::new(vault)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !prune.contains(&e.file_name().to_string_lossy().as_ref()))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|e| util::rel(vault, e.path()))
            .collect();
        files.sort();
        for f in files {
            let no_md = f.strip_suffix(".md").unwrap_or(&f).to_string();
            let b = f.rsplit('/').next().unwrap_or(&f).to_string();
            let b_no = b.strip_suffix(".md").unwrap_or(&b).to_string();
            byname.insert(b, f.clone());
            byname.insert(b_no, f.clone());
            exists.insert(no_md);
            exists.insert(f);
        }
        Index { exists, byname }
    }
    /// Link target → file path; None if missing ("." for an empty target).
    pub fn resolve(&self, vault: &Path, link: &str) -> Option<String> {
        let mut t = link;
        for sep in ['|', '#', '^'] {
            if let Some(i) = t.find(sep) {
                t = &t[..i];
            }
        }
        let t = t.trim();
        if t.is_empty() {
            return Some(".".into());
        }
        if self.exists.contains(t) {
            return Some(if vault.join(t).is_file() {
                t.to_string()
            } else {
                format!("{t}.md")
            });
        }
        self.byname.get(t).cloned()
    }
}

/// Markdown files under `root` the way ripgrep sees them (.gitignore respected, hidden skipped),
/// sorted. `skip` filters relative paths out.
fn rg_md(vault: &Path, root: &Path, skip: &dyn Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = ignore::WalkBuilder::new(root)
        .require_git(false)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_some_and(|t| t.is_file()) && e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.into_path())
        .filter(|p| !skip(&util::rel(vault, p)))
        .collect();
    v.sort();
    v
}

/// [[links]] in the files: (relative source, target), code spans ignored.
pub fn wikilinks(vault: &Path, files: &[PathBuf]) -> Vec<(String, String)> {
    static CODE: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    let code = CODE.get_or_init(|| Regex::new(r"`[^`]*`").unwrap());
    let link = LINK.get_or_init(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
    let mut out = Vec::new();
    for f in files {
        let rel = util::rel(vault, f);
        for l in util::read(f).lines().filter(|l| l.contains("[[")) {
            let t = code.replace_all(l, "");
            for c in link.captures_iter(&t) {
                out.push((rel.clone(), c[1].to_string()));
            }
        }
    }
    out
}

/// Frontmatter fields a wiki or project page must have; the missing ones (" title type …").
pub fn missing_fm(vault: &Path, rel: &str) -> String {
    let text = util::read(&vault.join(rel));
    let mut miss = String::new();
    for k in ["title", "type", "lang", "created", "updated"] {
        if fm::get(&text, k).is_none_or(|v| v.is_empty()) {
            miss.push(' ');
            miss.push_str(k);
        }
    }
    if rel.starts_with("wiki/sources/") {
        let mut lines = text.lines();
        let has = lines.next() == Some("---") && lines.take_while(|l| *l != "---").any(|l| l.contains("raw/"));
        if !has {
            miss.push_str(" sources(raw/…)");
        }
    }
    miss
}

fn has_topics(text: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?m)^Topics:.*\[\[wiki/topics/").unwrap())
        .is_match(text)
}

/// Daily-log 'learned:' lines with no [[wiki/…]] link: "file:line:text".
pub fn unlinked_learned(vault: &Path) -> Vec<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"^- [0-9:]+ \[[^\]]*\] learned:").unwrap());
    let mut out = Vec::new();
    for f in rg_md(vault, &vault.join("daily"), &|_| false) {
        let rel = util::rel(vault, &f);
        for (i, l) in util::read(&f).lines().enumerate() {
            if r.is_match(l) && !l.contains("[[wiki/") {
                out.push(format!("{rel}:{}:{l}", i + 1));
            }
        }
    }
    out
}

fn sorted_md(vault: &Path, dirs: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = dirs
        .iter()
        .flat_map(|d| {
            walkdir::WalkDir::new(vault.join(d))
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().ends_with(".md") && !e.file_type().is_dir())
                .map(|e| util::rel(vault, e.path()))
                .collect::<Vec<_>>()
        })
        .collect();
    v.sort();
    v
}

/// Returns (report, issue count).
pub fn lint_check(vault: &Path) -> (String, usize) {
    let idx = Index::build(vault);
    let mut out = String::new();
    let mut issues = 0;
    let mut inbound: HashSet<String> = HashSet::new();

    out.push_str("## Broken links\n");
    // Like rg --glob "!.kioku/**" --glob "!CLAUDE.md" --glob "!AGENTS.md" (basenames anywhere)
    let files = rg_md(vault, vault, &|r| {
        let base = r.rsplit('/').next().unwrap_or(r);
        r.starts_with(".kioku/") || base == "CLAUDE.md" || base == "AGENTS.md"
    });
    let mut broken = 0;
    for (src, link) in wikilinks(vault, &files) {
        if src.starts_with("raw/") {
            continue;
        }
        match idx.resolve(vault, &link) {
            None => {
                out.push_str(&format!("- {src} → [[{link}]]\n"));
                broken += 1;
            }
            Some(r) => {
                if src != r && src != "index.md" {
                    inbound.insert(r);
                }
            }
        }
    }
    if broken == 0 {
        out.push_str("(none)\n");
    }
    issues += broken;

    let pages = sorted_md(vault, &["wiki", "projects"]);
    out.push_str("## Orphan pages (no inbound links except from index)\n");
    let orphans: Vec<&String> = pages.iter().filter(|p| !inbound.contains(*p)).collect();
    for p in &orphans {
        out.push_str(&format!("- {p}\n"));
    }
    if orphans.is_empty() {
        out.push_str("(none)\n");
    }
    issues += orphans.len();

    out.push_str("## Not catalogued (neither in index.md nor in a topic hub)\n");
    let hubs: String = sorted_md(vault, &["wiki/topics"])
        .iter()
        .map(|h| util::read(&vault.join(h)))
        .collect::<Vec<_>>()
        .join("\n");
    let index = util::read(&vault.join("index.md"));
    let mut m = 0;
    for p in &pages {
        let key = format!("[[{}", p.strip_suffix(".md").unwrap_or(p));
        let in_hub = !p.starts_with("wiki/topics/") && hubs.contains(&key);
        if !index.contains(&key) && !in_hub {
            out.push_str(&format!("- {p}\n"));
            m += 1;
        }
    }
    if m == 0 {
        out.push_str("(none)\n");
    }
    issues += m;

    out.push_str("## No topic (wiki page without a 'Topics: [[wiki/topics/…]]' line)\n");
    let mut nt = 0;
    for p in pages.iter().filter(|p| p.starts_with("wiki/") && !p.starts_with("wiki/topics/")) {
        if !has_topics(&util::read(&vault.join(p))) {
            out.push_str(&format!("- {p}\n"));
            nt += 1;
        }
    }
    if nt == 0 {
        out.push_str("(none)\n");
    }
    issues += nt;

    out.push_str("## Missing frontmatter\n");
    let mut e = 0;
    for p in &pages {
        let miss = missing_fm(vault, p);
        if !miss.is_empty() {
            out.push_str(&format!("- {p}:{miss}\n"));
            e += 1;
        }
    }
    if e == 0 {
        out.push_str("(none)\n");
    }
    issues += e;

    out.push_str("## Learned, not in the wiki (daily 'learned:' lines without a [[wiki/…]] link)\n");
    let ln = unlinked_learned(vault);
    for l in &ln {
        out.push_str(&format!("- {}\n", util::take(l, 220)));
    }
    if ln.is_empty() {
        out.push_str("(none)\n");
    }
    issues += ln.len();

    out.push_str("## Calendar, todos and contacts\n");
    let vo = calendar::validate(vault);
    if vo.is_empty() {
        out.push_str("(none)\n");
    } else {
        for l in &vo {
            out.push_str(l);
            out.push('\n');
        }
        issues += vo.len();
    }

    out.push_str("## inbox\n");
    out.push_str(&format!("{} unprocessed file(s)\n", util::inbox_count(vault)));

    // Not counted as issues: checking whether facts still hold needs the user.
    out.push_str("## Due for review (facts not checked for a while; not counted as issues)\n");
    let due = stale::pages(vault, false);
    for d in due.iter().take(20) {
        out.push_str(&stale::line(d));
        out.push('\n');
    }
    if due.len() > 20 {
        out.push_str(&format!("- … {} more (kioku stale)\n", due.len() - 20));
    }
    if due.is_empty() {
        out.push_str("(none)\n");
    }
    out.push_str(&format!("\nTotal issues: {issues}\n"));
    (out, issues)
}

/// Problems in just-written vault files: "- file: problem" lines (empty when fine).
pub fn check_file(vault: &Path, args: &[String]) -> String {
    let vs = format!("{}/", vault.display());
    let mut files: Vec<String> = Vec::new();
    for a in args {
        let f = a.strip_prefix(&vs).unwrap_or(a);
        if f.starts_with('/') || !f.ends_with(".md") {
            continue;
        }
        if ["raw/", ".kioku/", ".obsidian/", ".git/", ".claude/", ".codex/"]
            .iter()
            .any(|p| f.starts_with(p))
            || f == "CLAUDE.md"
            || f == "AGENTS.md"
            || f == crate::home::FILE
        {
            continue;
        }
        if vault.join(f).is_file() {
            files.push(f.to_string());
        }
    }
    if files.is_empty() {
        return String::new();
    }
    let idx = Index::build(vault);
    let mut out = String::new();
    for f in &files {
        let text = util::read(&vault.join(f));
        let mut lines = text.lines();
        if lines.next() == Some("---") && !lines.any(|l| l == "---") {
            out.push_str(&format!("- {f}: the frontmatter is not closed (no second ---)\n"));
        }
        if f.starts_with("wiki/") || f.starts_with("projects/") {
            let miss = missing_fm(vault, f);
            if !miss.is_empty() {
                out.push_str(&format!("- {f}: missing frontmatter:{miss}\n"));
            }
            for p in stale::check(f, &text) {
                out.push_str(&format!("- {f}: {p}\n"));
            }
            if f.starts_with("wiki/") && !f.starts_with("wiki/topics/") && !has_topics(&text) {
                out.push_str(&format!("- {f}: no 'Topics: [[wiki/topics/…]]' line\n"));
            }
        }
    }
    let paths: Vec<PathBuf> = files.iter().map(|f| vault.join(f)).collect();
    for (src, link) in wikilinks(vault, &paths) {
        if idx.resolve(vault, &link).is_none() {
            out.push_str(&format!("- {src}: broken link [[{link}]]\n"));
        }
    }
    if files
        .iter()
        .any(|f| f.starts_with("calendar/") || f.starts_with("contacts/") || f.starts_with("journal/"))
    {
        let vo = calendar::validate(vault);
        for f in &files {
            for l in vo.iter().filter(|l| l.contains(&format!("- {f}"))) {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}
