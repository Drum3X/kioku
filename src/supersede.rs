//! Superseded knowledge. When a decision or preference in the work log is replaced by a newer
//! one, the old line's type becomes `superseded:` (in place, text kept) and the new line ends
//! with `(replaces YYYY-MM-DD HH:MM)`. A page that is out of date points to its successor with
//! `superseded_by: "[[…]]"` in its frontmatter (and `valid_until: YYYY-MM-DD` if known).
//! Superseded lines are left out of the session context, the deep context and search; a
//! superseded page ranks far lower in search and says what replaced it. Git keeps the history.
//!
//!   kioku supersedes        decisions that may replace an earlier one in the same project

use crate::text::words;
use crate::{fm, util};
use regex::Regex;
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

/// Whether a work-log line is a superseded entry.
pub fn is_line(l: &str) -> bool {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^- [0-9:]+ \[[^\]]*\] superseded:").unwrap()).is_match(l)
}

/// A page's successor (`superseded_by:`), if it has one.
pub fn page_successor(text: &str) -> Option<String> {
    let (data, _) = fm::parse(text);
    let s = data.get("superseded_by").map(fm::Val::str).unwrap_or_default();
    let s = s.trim().trim_matches(['"', '\'']).trim_start_matches("[[").trim_end_matches("]]");
    let s = s.split('|').next().unwrap_or("").trim();
    (!s.is_empty()).then(|| s.to_string())
}

#[derive(Clone)]
pub struct Entry {
    pub date: String,
    pub time: String,
    pub tag: String,
    pub ty: String,
    pub text: String,
}

/// Every decision and preference line still current, oldest first.
pub fn current(vault: &Path) -> Vec<Entry> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"^- ([0-9:]+) \[([^\]]*)\] (decision|preference): (.*)$").unwrap());
    let mut out = Vec::new();
    for f in util::md_files(&vault.join("daily")) {
        let t = util::read(&f);
        if fm::is_private(&t) {
            continue;
        }
        let date = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        for l in t.lines() {
            if let Some(c) = r.captures(l) {
                let tag = c[2].split('·').next().unwrap_or("").trim().to_lowercase();
                out.push(Entry { date: date.clone(), time: c[1].to_string(), tag, ty: c[3].to_string(), text: c[4].to_string() });
            }
        }
    }
    out.sort_by(|a, b| (a.date.as_str(), a.time.as_str()).cmp(&(b.date.as_str(), b.time.as_str())));
    out
}

/// Words that say little about what a decision is about.
const STOP: &str = " ve ile bir bu su o da de mi mu ne neden nasil icin gibi daha cok en ama veya ya ki her sey yok var ben sen biz siz olarak artik yine hem kadar sonra once yerine degil olmali olsun kalsin kalir kalacak tutuldu tutulur yapildi yapilir kullanici karari karar user agent the a an and or of to in on at for with from by is are was were be not no now only also still stay stays kept keep use uses used instead yerelde kioku projects ";

fn key_words(s: &str) -> HashSet<String> {
    // The link at the end and the commit references say nothing about the subject.
    let s = s.split(" ([[").next().unwrap_or(s);
    words(s)
        .into_iter()
        .filter(|w| w.chars().count() >= 4 && !w.chars().all(|c| c.is_ascii_hexdigit()) && !STOP.contains(&format!(" {w} ")))
        .collect()
}

fn shared(a: &str, b: &str) -> usize {
    key_words(a).intersection(&key_words(b)).count()
}

/// Similarity by meaning from which two entries sharing a couple of key words may be about the
/// same thing. (On this kind of short line multilingual-e5-small scores most pairs of one
/// project 0.85–0.92, so meaning alone does not separate them; the shared words do the rest.)
const SIMILAR: f32 = 0.90;

fn cos(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Whether `new` may replace `old`: with vectors (the model), similar in meaning and two or
/// more shared key words; without them, four or more shared key words.
fn related(old: &Entry, new: &Entry, vs: Option<(&[f32], &[f32])>) -> bool {
    let w = shared(&old.text, &new.text);
    match vs {
        Some((a, b)) => w >= 2 && cos(a, b) >= SIMILAR,
        None => w >= 4,
    }
}

/// Vectors of the entries' texts, when `model` and the model is there.
fn vectors(es: &[&Entry], model: bool) -> Option<Vec<Vec<f32>>> {
    if !model {
        return None;
    }
    crate::semantic::embed(&es.iter().map(|e| e.text.clone()).collect::<Vec<_>>())
}

pub fn line(e: &Entry) -> String {
    format!("{} {} [{}] {}: {}", e.date, e.time, e.tag, e.ty, util::take(&e.text, 200))
}

/// Pairs (older, newer) of current entries of the same project and type that may be about the
/// same thing, the newer one written in the last `days` days and not already saying what it
/// replaces (`(replaces …)`).
pub fn candidates(vault: &Path, days: i64, model: bool) -> Vec<(Entry, Entry)> {
    let es = current(vault);
    let since = (util::today() - chrono::Duration::days(days)).format("%Y-%m-%d").to_string();
    let mut pairs = Vec::new();
    let vs = vectors(&es.iter().collect::<Vec<_>>(), model);
    let mut used = HashSet::new();
    for j in (0..es.len()).rev() {
        let b = &es[j];
        if b.date < since || b.text.contains("(replaces ") {
            continue;
        }
        let v = |i: usize| vs.as_ref().map(|v| (v[i].as_slice(), v[j].as_slice()));
        if let Some(i) = (0..j).rev().find(|&i| !used.contains(&i) && es[i].tag == b.tag && es[i].ty == b.ty && related(&es[i], b, v(i))) {
            used.insert(i);
            pairs.push((i, j));
        }
    }
    pairs.sort();
    pairs.into_iter().map(|(i, j)| (es[i].clone(), es[j].clone())).collect()
}

/// For the write check: the decision or preference just written (the last line of a daily
/// file) may replace an earlier one of the same project.
pub fn nudge(vault: &Path, rel: &str, text: &str) -> Option<String> {
    if !rel.starts_with("daily/") {
        return None;
    }
    let last = text.lines().rev().find(|l| !l.trim().is_empty())?;
    if last.contains("(replaces ") || !(last.contains("] decision: ") || last.contains("] preference: ")) {
        return None;
    }
    let date = rel.trim_start_matches("daily/").trim_end_matches(".md");
    let es = current(vault);
    let me = es.iter().rposition(|e| e.date == date && last.starts_with(&format!("- {} ", e.time)) && last.contains(&format!("{}: {}", e.ty, e.text)))?;
    let b = &es[me];
    let older: Vec<&Entry> = es[..me].iter().filter(|a| a.tag == b.tag && a.ty == b.ty).collect();
    if older.is_empty() {
        return None;
    }
    let mut all = older.clone();
    all.push(b);
    let vs = vectors(&all, crate::semantic::ready());
    let n = older.len();
    let hits: Vec<usize> = (0..n).filter(|&i| related(older[i], b, vs.as_ref().map(|v| (v[i].as_slice(), v[n].as_slice())))).collect();
    if hits.is_empty() {
        return None;
    }
    let lines: String = hits.iter().rev().take(3).map(|&i| format!("  - {}\n", line(older[i]))).collect();
    Some(format!(
        "- {rel}: this {} may replace an earlier one of [{}]:\n{lines}  If it does, change that line's `{}:` to `superseded:` in its daily file (keep the text) and end the new line with `(replaces DATE HH:MM)`. If both still hold, leave them.\n",
        b.ty, b.tag, b.ty
    ))
}
pub fn run(vault: &Path, _args: &[String]) -> anyhow::Result<i32> {
    let c = candidates(vault, 30, crate::semantic::ready());
    if c.is_empty() {
        println!("No decision or preference of the last 30 days looks like it replaces an earlier one.");
        return Ok(0);
    }
    for (a, b) in &c {
        println!("- older: {}\n  newer: {}", line(a), line(b));
    }
    println!("\nFor each pair: if the newer one replaces the older, change the older line's type to `superseded:` (keep the text) and end the newer line with `(replaces DATE HH:MM)`. Pairs that only share a subject stay as they are.");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_key_words() {
        assert_eq!(shared("DAV şifresi KWallet yerine dav.pass dosyasında (chmod 600) ([[projects/kioku]])", "DAV şifresi anahtar zincirinde; yoksa chmod 600 dosya ([[projects/kioku]])"), 2);
        assert_eq!(shared("Oracle notunun paths alanı boş kalıyor", "hibrit arama kurulumda kapalı gelsin"), 0);
    }
    #[test]
    fn page_successor_reads_the_link() {
        assert_eq!(page_successor("---\ntitle: X\nsuperseded_by: \"[[wiki/concepts/y|Y]]\"\n---\n# X\n").as_deref(), Some("wiki/concepts/y"));
        assert_eq!(page_successor("---\ntitle: X\n---\n"), None);
    }
    #[test]
    fn nudges_and_lists_a_newer_decision() {
        let v = std::env::temp_dir().join(format!("kioku-sup-{}", std::process::id()));
        let d = v.join("daily");
        std::fs::create_dir_all(&d).unwrap();
        let today = util::today().format("%Y-%m-%d").to_string();
        std::fs::write(d.join("2026-01-10.md"), "# a\n- 10:00 [kioku · a@h] decision: hibrit arama varsayılan olarak kapalı gelsin, e5 modeli kurulumda indirilmesin\n- 11:00 [config] decision: hibrit arama kapalı\n").unwrap();
        let new = "# b\n- 09:00 [kioku · b@h] decision: hibrit arama her kurulumda içinde, e5 modeli kurulumda indiriliyor, arama kapalı\n";
        std::fs::write(d.join(format!("{today}.md")), new).unwrap();
        let rel = format!("daily/{today}.md");
        assert!(nudge(&v, &rel, new).is_some_and(|m| m.contains("2026-01-10 10:00") && !m.contains("[config]")));
        assert_eq!(candidates(&v, 30, false).len(), 1);
        // Marked: the old line is superseded, the new one says so; nothing more to propose.
        std::fs::write(d.join("2026-01-10.md"), "# a\n- 10:00 [kioku · a@h] superseded: hibrit arama varsayılan olarak kapalı gelsin, e5 modeli kurulumda indirilmesin\n").unwrap();
        assert!(nudge(&v, &rel, new).is_none());
        assert!(candidates(&v, 30, false).is_empty());
        assert!(is_line("- 10:00 [kioku · a@h] superseded: x"));
        std::fs::remove_dir_all(&v).unwrap();
    }
}
