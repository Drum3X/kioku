//! Corrections: `correction:` lines in the work log, written when the user corrects an agent
//! ("no, not like that", "why is this in English"). When the same kind of correction comes up a
//! second time (in any agent), it should become a standing rule in AGENTS.md (or Core.md, for
//! something about the user), with the user's yes. The lines are then marked `ruled:` (made a
//! rule) or `declined:` (the user said no), and are not proposed again.
//!
//!   kioku corrections        repeated corrections not yet turned into a rule

use crate::text::words;
use crate::{fm, util};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

pub struct Correction {
    pub date: String,
    pub time: String,
    pub by: String,
    pub text: String,
}

/// Words that say little about what was corrected.
const STOP: &str = " ve ile bir bu su o da de mi mu ne neden nasil icin gibi daha cok en ama veya ya ki her sey yok var ben sen biz siz kullanici user agent ajan the a an and or of to in on at for with from by is are was were be not no did does should yerine degil olmali istedi istiyor said says asked wrong instead ";

fn key_words(s: &str) -> HashSet<String> {
    words(s)
        .into_iter()
        .filter(|w| w.chars().count() >= 3 && !STOP.contains(&format!(" {w} ")))
        .collect()
}
/// Whether two corrections are about the same thing: at least two shared key words and a third
/// of the smaller set.
pub fn similar(a: &str, b: &str) -> bool {
    let (x, y) = (key_words(a), key_words(b));
    let shared = x.intersection(&y).count();
    let small = x.len().min(y.len()).max(1);
    shared >= 2 && shared * 3 >= small
}

/// Every `correction:` line still open (not ruled or declined), oldest first.
pub fn open(vault: &Path) -> Vec<Correction> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"^- ([0-9:]+) \[([^\]]*)\] correction: (.*)$").unwrap());
    let mut out = Vec::new();
    for f in util::md_files(&vault.join("daily")) {
        let t = util::read(&f);
        if fm::is_private(&t) {
            continue;
        }
        let date = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        for l in t.lines() {
            if let Some(c) = r.captures(l) {
                let by = c[2].split_once('·').map(|x| x.1.trim().to_string()).unwrap_or_default();
                out.push(Correction { date: date.clone(), time: c[1].to_string(), by, text: c[3].to_string() });
            }
        }
    }
    out.sort_by(|a, b| (a.date.as_str(), a.time.as_str()).cmp(&(b.date.as_str(), b.time.as_str())));
    out
}

/// Groups of two or more open corrections about the same thing.
pub fn repeated(vault: &Path) -> Vec<Vec<Correction>> {
    let mut left = open(vault);
    let mut groups = Vec::new();
    while !left.is_empty() {
        let first = left.remove(0);
        let (same, rest): (Vec<Correction>, Vec<Correction>) = left.into_iter().partition(|c| similar(&first.text, &c.text));
        left = rest;
        if !same.is_empty() {
            let mut g = vec![first];
            g.extend(same);
            groups.push(g);
        }
    }
    groups
}

pub fn line(c: &Correction) -> String {
    let by = if c.by.is_empty() { String::new() } else { format!(" ({})", c.by) };
    format!("{} {}{by}: {}", c.date, c.time, util::take(&c.text, 220))
}

/// For the write check: a correction in `text` (a daily file just written) that repeats an
/// earlier open one. The message asks the agent to propose a rule.
pub fn nudge(vault: &Path, rel: &str, text: &str) -> Option<String> {
    if !rel.starts_with("daily/") || !text.contains("] correction: ") {
        return None;
    }
    let groups = repeated(vault);
    let date = rel.trim_start_matches("daily/").trim_end_matches(".md");
    let hit = groups.into_iter().find(|g| g.iter().any(|c| c.date == date))?;
    let lines: String = hit.iter().map(|c| format!("  - {}\n", line(c))).collect();
    Some(format!(
        "- {rel}: this correction has come up before:\n{lines}  Ask the user whether to make it a standing rule: propose one short line for AGENTS.md › Personal rules (or Core.md, if it is about the user), and write it only with their yes. Then change these lines' `correction:` to `ruled:` (or `declined:` if they said no).\n"
    ))
}

pub fn run(vault: &Path, _args: &[String]) -> anyhow::Result<i32> {
    let groups = repeated(vault);
    if groups.is_empty() {
        let n = open(vault).len();
        println!("No repeated corrections ({n} open correction(s), each seen once).");
        return Ok(0);
    }
    for g in &groups {
        println!("Seen {} times:", g.len());
        for c in g {
            println!("  - {}", line(c));
        }
    }
    println!("\nPropose a standing rule for each (AGENTS.md › Personal rules, or Core.md about the user), with the user's yes; then mark the lines ruled: or declined:.");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn similar_corrections() {
        assert!(similar(
            "soru penceresi İngilizce çıktı; sorular ve butonlar da Türkçe olmalı",
            "AskUserQuestion seçenekleri İngilizce yazıldı, kullanıcı Türkçe istedi"
        ));
        assert!(!similar("emoji kullanıldı", "commit mesajı çok uzun"));
        assert!(!similar("Türkçe cevap ver", "cevap çok uzun"));
    }
    #[test]
    fn repeats_are_found_and_nudged() {
        let v = std::env::temp_dir().join(format!("kioku-corr-{}", std::process::id()));
        let d = v.join("daily");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("2026-11-20.md"), "# x\n- 10:00 [k · a@h] correction: soru penceresi İngilizce çıktı, Türkçe olmalı\n- 11:00 [k] correction: emoji kullanıldı\n").unwrap();
        let today = "# y\n- 09:00 [k · b@h] correction: butonlar İngilizce yazıldı, kullanıcı Türkçe istedi\n";
        std::fs::write(d.join("2026-11-21.md"), today).unwrap();
        let g = repeated(&v);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].len(), 2);
        assert!(nudge(&v, "daily/2026-11-21.md", today).is_some_and(|m| m.contains("standing rule")));
        // Once ruled, it is not proposed again.
        std::fs::write(d.join("2026-11-21.md"), today.replace("correction:", "ruled:")).unwrap();
        assert!(repeated(&v).is_empty());
        std::fs::remove_dir_all(&v).unwrap();
    }
}
