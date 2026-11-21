//! `kioku memories [--done ID…]`: what the agents on this machine keep in their own memory
//! (Claude Code's auto memory, Hermes's MEMORY.md and USER.md, Codex's memories), compared
//! with the vault. Each entry is either already in the vault (Core.md, AGENTS.md or a page) or
//! known to that one agent only. For the latter the agent proposes where it belongs, with the
//! user's yes for Core.md and AGENTS.md, then marks it handled with `--done ID` (also when it is
//! a working note that should stay local). The local files are only read, never changed.

use crate::text::words;
use crate::{search, util};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct Entry {
    pub id: String,
    pub agent: &'static str,
    pub file: PathBuf,
    pub title: String,
    pub text: String,
}

fn handled_file() -> PathBuf {
    util::state_dir().join("memories-handled")
}
fn handled() -> HashSet<String> {
    util::read(&handled_file()).lines().map(|l| l.split_whitespace().next().unwrap_or("").to_string()).collect()
}
fn id_of(agent: &str, text: &str) -> String {
    let h = Sha256::digest(format!("{agent}\n{}", text.trim()).as_bytes());
    h.iter().take(4).map(|b| format!("{b:02x}")).collect()
}

/// A memory file with YAML frontmatter (Claude Code): its name/description and body.
fn md_entry(agent: &'static str, f: &Path) -> Option<Entry> {
    let t = util::read(f);
    let (data, body) = crate::fm::parse(&t);
    let get = |k: &str| data.get(k).map(crate::fm::Val::str).unwrap_or_default();
    let title = [get("description"), get("name")].into_iter().find(|s| !s.is_empty()).unwrap_or_else(|| {
        f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    });
    let text = format!("{title}\n{}", body.trim());
    if body.trim().is_empty() && title.is_empty() {
        return None;
    }
    Some(Entry { id: id_of(agent, &text), agent, file: f.to_path_buf(), title, text })
}

/// Every local memory entry of the agents on this machine.
pub fn entries() -> Vec<Entry> {
    let home = util::home();
    let mut out = Vec::new();
    // Claude Code: ~/.claude/projects/<project>/memory/*.md (MEMORY.md is only the index).
    for d in std::fs::read_dir(home.join(".claude/projects")).into_iter().flatten().flatten() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(d.path().join("memory"))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md") && p.file_name().is_some_and(|n| n != "MEMORY.md"))
            .collect();
        files.sort();
        out.extend(files.iter().filter_map(|f| md_entry("claude", f)));
    }
    // Codex: ~/.codex/memories/**/*.md
    let cx = std::env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".codex"));
    let mut cf: Vec<PathBuf> = walkdir::WalkDir::new(cx.join("memories"))
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.path().to_path_buf())
        .collect();
    cf.sort();
    out.extend(cf.iter().filter_map(|f| md_entry("codex", f)));
    // Hermes: <HERMES_HOME>/memories/MEMORY.md and USER.md, entries separated by a § line.
    for name in ["USER.md", "MEMORY.md"] {
        let f = util::hermes_home().join("memories").join(name);
        for part in util::read(&f).split("\n§\n") {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let title = util::take(p.lines().next().unwrap_or(""), 120);
            out.push(Entry { id: id_of("hermes", p), agent: "hermes", file: f.clone(), title, text: p.to_string() });
        }
    }
    out
}

/// Where in the vault an entry already is: the page holding most of its key words (Core.md,
/// AGENTS.md, or the best search results), when that is at least 60 %.
pub fn covered(vault: &Path, e: &Entry) -> Option<String> {
    let keys: Vec<String> = {
        let mut seen = HashSet::new();
        words(&e.text)
            .into_iter()
            .filter(|w| w.chars().count() >= 4 && !w.chars().all(|c| c.is_ascii_digit()) && seen.insert(w.clone()))
            .take(14)
            .collect()
    };
    if keys.len() < 3 {
        return None;
    }
    let mut cands: Vec<String> = vec!["Core.md".into(), "AGENTS.md".into()];
    cands.extend(search::top(vault, &keys, 3).into_iter().map(|r| r.0));
    let mut best: Option<(f64, String)> = None;
    for c in cands {
        let have: HashSet<String> = words(&util::read(&vault.join(&c))).into_iter().collect();
        if have.is_empty() {
            continue;
        }
        let share = keys.iter().filter(|k| have.iter().any(|h| h.starts_with(k.as_str()) || k.starts_with(h.as_str()) && h.chars().count() >= 4)).count() as f64
            / keys.len() as f64;
        if best.as_ref().is_none_or(|b| share > b.0) {
            best = Some((share, c));
        }
    }
    best.filter(|b| b.0 >= 0.6).map(|b| b.1)
}

/// Similarity (multilingual-e5-small cosine) from which an entry counts as already in the vault.
pub const SIMILAR: f32 = SEMANTIC_THRESHOLD;
const SEMANTIC_THRESHOLD: f32 = 0.84;

/// Open entries (not handled, or all) and, for each, the vault page that already holds it: by
/// meaning when `semantic` and the model is there (it works across languages), else by words.
pub fn list(vault: &Path, all: bool, semantic: bool) -> Vec<(Entry, Option<String>, Option<f32>)> {
    let done = handled();
    let es: Vec<Entry> = entries().into_iter().filter(|e| all || !done.contains(&e.id)).collect();
    let sem = if semantic && !es.is_empty() {
        search::semantic_matches(vault, &es.iter().map(|e| e.text.clone()).collect::<Vec<_>>())
    } else {
        None
    };
    es.into_iter()
        .enumerate()
        .map(|(i, e)| match &sem {
            Some(m) => {
                let (page, sim) = m[i].clone();
                let c = (sim >= SIMILAR).then_some(page).or_else(|| covered(vault, &e));
                (e, c, Some(sim))
            }
            None => {
                let c = covered(vault, &e);
                (e, c, None)
            }
        })
        .collect()
}
pub fn open(vault: &Path) -> Vec<(Entry, Option<String>)> {
    list(vault, false, false).into_iter().map(|(e, c, _)| (e, c)).collect()
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    if args.first().map(String::as_str) == Some("--done") {
        let ids: Vec<&String> = args[1..].iter().collect();
        if ids.is_empty() {
            return util::die("usage: kioku memories --done ID …");
        }
        let all = entries();
        let mut add = String::new();
        for id in ids {
            match all.iter().find(|e| &e.id == id) {
                Some(e) => add.push_str(&format!("{id} {} {}\n", e.agent, util::take(&e.title, 80))),
                None => return util::die(format!("no local memory entry {id} (see kioku memories)")),
            }
        }
        let cur = util::read(&handled_file());
        util::write(&handled_file(), &format!("{cur}{add}"))?;
        print!("marked handled:\n{add}");
        return Ok(0);
    }
    let all = args.iter().any(|a| a == "--all");
    let semantic = args.iter().any(|a| a == "--semantic");
    let list: Vec<(Entry, Option<String>, Option<f32>)> = list(vault, all, semantic);
    let sim = |s: &Option<f32>| s.map(|x| format!(" (similarity {x:.2})")).unwrap_or_default();
    if semantic && list.iter().all(|x| x.2.is_none()) && !list.is_empty() {
        eprintln!("kioku: --semantic needs a build with hybrid search and its model (kioku semantic status); matching by words");
    }
    if list.is_empty() {
        println!("No local memories to look at on this machine ({}).", util::host());
        return Ok(0);
    }
    let (only, inv): (Vec<_>, Vec<_>) = list.iter().partition(|(_, c, _)| c.is_none());
    if !only.is_empty() {
        println!("Known to one agent only ({}):", util::host());
        for (e, _, s) in &only {
            println!("- [{}] {}: {}{}\n    {}", e.id, e.agent, e.title, sim(s), util::slash(&e.file));
        }
    }
    if !inv.is_empty() {
        println!("Already in the vault (mark them handled):");
        for (e, c, s) in &inv {
            println!("- [{}] {}: {} → {}{}", e.id, e.agent, util::take(&e.title, 100), c.as_deref().unwrap_or(""), sim(s));
        }
    }
    if !semantic {
        println!("\n(Matched by words: an entry written in another language than the vault may be there anyway; check before copying it, or use --semantic.)");
    }
    println!(
        "\nFor each entry known to one agent: if every agent should know it, propose where it goes (about the user: Core.md; a working rule: AGENTS.md › Personal rules; both only with the user's yes; knowledge or a decision: a wiki page or the daily log); a note only for that agent stays local. Then: kioku memories --done ID …"
    );
    Ok(0)
}
