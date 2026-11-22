//! `kioku search`: ranked multi-term search (BM25 over title ×3, headings, tags, aliases and
//! keys ×2, body ×1,
//! times the share of terms matched, a page-type weight and a small recency bonus). Matching
//! folds case and Turkish letters and allows suffixes both ways.
//!
//! A page is ranked as a whole. Its sections (`##` and `###` headings) are scored too, only to
//! show which section matches best, so that an agent can go straight to it in a long page.
//! (Ranking by the best section was measured with `kioku eval` and did not help: 2026-11-20.)

use crate::text::{fold, word_re, words};
use crate::util;
use chrono::{NaiveDate, Utc};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const STOP_WORDS: &str = "
  ve ile bir bu şu o da de mi mı mu mü ne neden niye nasıl hangi hangisi hangisiydi için gibi daha çok en
  ama veya ya ki her şey neydi nedir var yok ben sen biz siz benim bana beni bizim olan oldu olarak kadar
  diye mı acaba şimdi zaten artık hiç
  the a an and or of to in on at for with from by what how why which who did do does we i you my our is are
  was were be been it this that about can should would
";

#[derive(Clone)]
enum Term {
    Word(String),
    Phrase(String),
}
impl Term {
    fn key(&self) -> &str {
        match self {
            Term::Word(s) | Term::Phrase(s) => s,
        }
    }
}

/// 1 exact, 0.8 with a suffix either way, 0 none.
fn word_match(w: &str, d: &str) -> f64 {
    if d == w {
        return 1.0;
    }
    let (wl, dl) = (w.chars().count(), d.chars().count());
    if wl >= 3 && d.starts_with(w) {
        return 0.8;
    }
    if dl >= 4 && w.starts_with(d) && wl - dl <= 4 {
        return 0.8;
    }
    0.0
}

/// Word counts of some text, per field (title, headings and tags, body).
#[derive(Default)]
struct Part {
    counts: [HashMap<String, usize>; 3],
    folded: [String; 3],
    len: usize,
    /// The section's heading and its line in the body (none for the header and the intro).
    heading: String,
    line: Option<usize>,
}
impl Part {
    fn new(fields: &[Vec<String>; 3], heading: String, line: Option<usize>) -> Part {
        let mut p = Part { heading, line, ..Default::default() };
        for (k, f) in fields.iter().enumerate() {
            let joined = f.join("\n");
            for w in words(&joined) {
                *p.counts[k].entry(w).or_insert(0) += 1;
                p.len += 1;
            }
            p.folded[k] = fold(&joined);
        }
        p
    }
}

struct Doc {
    rel: String,
    title: String,
    private: bool,
    date: Option<String>,
    body: Vec<String>,
    body_start: usize,
    /// Title, tags, aliases and the `#` heading: counted with every section.
    header: Part,
    /// The intro before the first section, then one part per `##`/`###` section.
    parts: Vec<Part>,
    len: usize,
    /// The page that replaces this one (`superseded_by:`).
    successor: Option<String>,
}

fn walk(p: &Path, out: &mut Vec<PathBuf>) {
    let Ok(md) = std::fs::metadata(p) else { return };
    if md.is_dir() {
        let Ok(rd) = std::fs::read_dir(p) else { return };
        for e in rd.flatten() {
            if !e.file_name().to_string_lossy().starts_with('.') {
                walk(&e.path(), out);
            }
        }
    } else if p.to_string_lossy().ends_with(".md") {
        out.push(p.to_path_buf());
    }
}

fn strip_quotes(v: &str) -> String {
    let v = v.strip_prefix(['"', '\'']).unwrap_or(v);
    v.strip_suffix(['"', '\'']).unwrap_or(v).to_string()
}

fn parse(vault: &Path, file: &Path) -> Doc {
    let text = util::read(file);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut fm: HashMap<String, String> = HashMap::new();
    let mut i = 0;
    if lines.first() == Some(&"---") {
        i = 1;
        while i < lines.len() && lines[i] != "---" {
            let l = lines[i];
            if let Some(c) = l.find(':') {
                let k = &l[..c];
                if !k.is_empty() && k.chars().all(|ch| ch.is_ascii_alphabetic() || ch == '_') {
                    fm.insert(k.to_string(), strip_quotes(l[c + 1..].trim_start()));
                }
            }
            i += 1;
        }
        i += 1;
    }
    // Superseded work-log lines stay in the file (and git) but are not searched.
    let body: Vec<String> = lines.iter().skip(i).map(|s| if crate::supersede::is_line(s) { String::new() } else { s.to_string() }).collect();
    let successor = crate::supersede::page_successor(&text);
    let title = fm.get("title").cloned().unwrap_or_default();
    let mut head: [Vec<String>; 3] = [
        vec![title.clone()],
        vec![
            fm.get("tags").cloned().unwrap_or_default(),
            fm.get("aliases").cloned().unwrap_or_default(),
            fm.get("keys").cloned().unwrap_or_default(),
        ],
        vec![],
    ];
    let mut parts = Vec::new();
    let mut cur: [Vec<String>; 3] = Default::default();
    let (mut cur_head, mut cur_line) = (String::new(), None);
    for (n, l) in body.iter().enumerate() {
        if let Some(t) = l.strip_prefix("# ") {
            head[0].push(t.to_string());
        } else if is_heading2(l) {
            let h = l.trim_start_matches('#');
            let h = h.strip_prefix(' ').unwrap_or(h).to_string();
            if l.starts_with("## ") || l.starts_with("### ") {
                parts.push(Part::new(&cur, std::mem::take(&mut cur_head), cur_line));
                cur = Default::default();
                cur_head = h.clone();
                cur_line = Some(n);
            }
            cur[1].push(h);
        } else {
            cur[2].push(l.clone());
        }
    }
    parts.push(Part::new(&cur, cur_head, cur_line));
    let header = Part::new(&head, String::new(), None);
    let len = header.len + parts.iter().map(|p| p.len).sum::<usize>();
    let base = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let date = ["updated", "date", "created"]
        .iter()
        .find_map(|k| fm.get(*k).filter(|v| !v.is_empty()).cloned())
        .or_else(|| {
            (base.len() >= 10
                && base.as_bytes()[..10]
                    .iter()
                    .enumerate()
                    .all(|(i, b)| if i == 4 || i == 7 { *b == b'-' } else { b.is_ascii_digit() }))
            .then(|| base[..10].to_string())
        });
    Doc {
        rel: util::rel(vault, file),
        title,
        private: fm.get("private").map(String::as_str) == Some("true"),
        date,
        body,
        body_start: i,
        header,
        parts,
        len,
        successor,
    }
}
/// `## ` … `###### ` heading.
fn is_heading2(l: &str) -> bool {
    let n = l.bytes().take_while(|b| *b == b'#').count();
    (2..=6).contains(&n) && l.as_bytes().get(n) == Some(&b' ')
}

const FIELD_W: [f64; 3] = [3.0, 2.0, 1.0];

#[allow(clippy::needless_range_loop)]
fn term_freq(d: &Part, t: &Term) -> f64 {
    let mut tf = 0.0;
    for k in 0..3 {
        match t {
            Term::Phrase(p) => tf += FIELD_W[k] * d.folded[k].matches(p.as_str()).count() as f64,
            Term::Word(w) => {
                for (tok, c) in &d.counts[k] {
                    let m = word_match(w, tok);
                    if m > 0.0 {
                        tf += FIELD_W[k] * m * *c as f64;
                    }
                }
            }
        }
    }
    tf
}

fn type_weight(rel: &str) -> f64 {
    if rel == "Core.md" || rel.starts_with("projects/") {
        1.2
    } else if rel.starts_with("wiki/topics/") {
        1.0
    } else if rel.starts_with("wiki/") {
        1.2
    } else if rel.starts_with("daily/") || rel.starts_with("journal/") {
        0.85
    } else if rel == "index.md" {
        0.6
    } else if rel.starts_with("raw/") {
        0.7
    } else {
        1.0
    }
}
fn recency(d: &Doc) -> f64 {
    let Some(ds) = &d.date else { return 1.0 };
    let Ok(day) = NaiveDate::parse_from_str(&util::take(ds, 10), "%Y-%m-%d") else {
        return 1.0;
    };
    let t = day.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let days = (Utc::now() - t).num_milliseconds() as f64 / 86_400_000.0;
    if days <= 14.0 {
        1.05
    } else if days <= 90.0 {
        1.02
    } else {
        1.0
    }
}

/// Ranked pages for the terms: (query terms, parsed pages, (page index, score, terms matched)),
/// best first. None when the query is empty after stop words or the vault has no pages.
/// (page index, score, terms matched, best section)
type Hit = (usize, f64, usize, usize);

fn rank(vault: &Path, terms: &[String], raw: bool, question: Option<&str>) -> Option<(Vec<Term>, Vec<Doc>, Vec<Hit>)> {
    let stop: HashSet<String> = words(STOP_WORDS).into_iter().collect();
    let mut query: Vec<Term> = Vec::new();
    for t in terms {
        let f = fold(t).trim().to_string();
        if f.is_empty() {
            continue;
        }
        let ws: Vec<&str> = word_re().find_iter(&f).map(|m| m.as_str()).collect();
        if ws.len() == 1 && ws[0] == f {
            query.push(Term::Word(f));
        } else if !ws.is_empty() {
            query.push(Term::Phrase(f));
        }
    }
    let mut q: Vec<Term> = query
        .iter()
        .filter(|t| matches!(t, Term::Phrase(_)) || !stop.contains(t.key()))
        .cloned()
        .collect();
    if q.is_empty() {
        q = query;
    }
    let mut seen = HashSet::new();
    q.retain(|t| seen.insert(t.key().to_string()));
    // A time expression ("geçen hafta", "dün"): its words are not search terms; pages from that
    // period come first below.
    let when = period(terms, question);
    if when.is_some() {
        q.retain(|t| !t.key().split_whitespace().all(crate::when::is_time_word));
    }
    if q.is_empty() && when.is_none() {
        return None;
    }

    let mut dirs = vec![
        "Core.md", "projects", "wiki", "daily", "journal", "calendar", "contacts", "reviews", "index.md",
    ];
    if raw {
        dirs.push("raw");
    }
    let mut files = Vec::new();
    for d in dirs {
        walk(&vault.join(d), &mut files);
    }
    let docs: Vec<Doc> = files.iter().map(|f| parse(vault, f)).collect();
    if docs.is_empty() {
        return None;
    }
    let n = docs.len() as f64;
    let avg = docs.iter().map(|d| d.len as f64).sum::<f64>() / n;
    let nparts = docs.iter().map(|d| d.parts.len()).sum::<usize>() as f64;
    let avg_part = docs
        .iter()
        .flat_map(|d| d.parts.iter().map(|p| (p.len + d.header.len) as f64))
        .sum::<f64>()
        / nparts.max(1.0);
    let (k1, b) = (1.2, 0.75);
    // Per page: the header's term frequencies, each section's, and the page's (their sum).
    let htf: Vec<Vec<f64>> = docs.iter().map(|d| q.iter().map(|t| term_freq(&d.header, t)).collect()).collect();
    let ptf: Vec<Vec<Vec<f64>>> = docs
        .iter()
        .map(|d| d.parts.iter().map(|p| q.iter().map(|t| term_freq(p, t)).collect()).collect())
        .collect();
    let tfs: Vec<Vec<f64>> = (0..docs.len())
        .map(|i| (0..q.len()).map(|j| htf[i][j] + ptf[i].iter().map(|p| p[j]).sum::<f64>()).collect())
        .collect();
    let idf: Vec<f64> = (0..q.len())
        .map(|j| {
            let df = tfs.iter().filter(|r| r[j] > 0.0).count() as f64;
            (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
        })
        .collect();

    // BM25 times the share of terms matched, for term frequencies over a text of length len.
    let score = |tf: &dyn Fn(usize) -> f64, len: f64, avg: f64| {
        let (mut s, mut matched) = (0.0, 0);
        for (j, w) in idf.iter().enumerate() {
            let tf = tf(j);
            if tf == 0.0 {
                continue;
            }
            matched += 1;
            s += w * (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * len / avg));
        }
        (s * matched as f64 / q.len() as f64, matched)
    };
    let mut results: Vec<Hit> = Vec::new();
    for (i, d) in docs.iter().enumerate() {
        let (whole, matched) = score(&|j| tfs[i][j], d.len as f64, avg);
        if matched == 0 {
            continue;
        }
        let (mut best, mut at) = (0.0, 0);
        // The section that matches best, for the output.
        for (k, p) in d.parts.iter().enumerate() {
            let (s, _) = score(&|j| htf[i][j] + ptf[i][k][j], (p.len + d.header.len) as f64, avg_part);
            if s > best {
                (best, at) = (s, k);
            }
        }
        let s = whole * type_weight(&d.rel) * recency(d);
        results.push((i, s, matched, at));
    }
    results.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| docs[a.0].rel.cmp(&docs[b.0].rel))
    });
    // Hybrid search, when switched on: fuse with the sections ranked by meaning.
    let query = question.map(str::to_string).unwrap_or_else(|| terms.join(" "));
    if let Some(vec) = crate::semantic::rank(vault, &section_texts(&docs), &query) {
        let bm: Vec<usize> = results.iter().map(|r| r.0).collect();
        let vd: Vec<usize> = vec.iter().map(|v| v.0).collect();
        results = crate::semantic::fuse(&bm, &vd)
            .into_iter()
            .map(|(d, s)| match results.iter().find(|r| r.0 == d) {
                Some(r) => (d, s * 100.0, r.2, r.3),
                None => (d, s * 100.0, 0, vec.iter().find(|v| v.0 == d).map(|v| v.1).unwrap_or(0)),
            })
            .collect();
    }
    // A superseded page ranks far lower: what replaced it should come first.
    if docs.iter().any(|d| d.successor.is_some()) {
        for h in results.iter_mut().filter(|h| docs[h.0].successor.is_some()) {
            h.1 *= 0.3;
        }
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }
    if let Some(r) = when {
        let kind = |rel: &str| {
            if rel.starts_with("reviews/") {
                0
            } else if rel.starts_with("daily/") {
                1
            } else if rel.starts_with("calendar/") {
                2
            } else {
                3
            }
        };
        let mut first: Vec<Hit> = docs
            .iter()
            .enumerate()
            .filter(|(_, d)| crate::when::span(&d.rel).is_some_and(|s| crate::when::overlaps(s, r)))
            .map(|(i, _)| results.iter().find(|h| h.0 == i).copied().unwrap_or((i, 0.0, 0, 0)))
            .collect();
        // Within the period: by score (a review, which sums the period up, counts 1.5×), then
        // reviews, daily notes and events before the rest, newest first.
        let w = |h: &Hit| h.1 * if kind(&docs[h.0].rel) == 0 { 1.5 } else { 1.0 };
        first.sort_by(|a, b| {
            w(b).partial_cmp(&w(a))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(kind(&docs[a.0].rel).cmp(&kind(&docs[b.0].rel)))
                .then(docs[b.0].rel.cmp(&docs[a.0].rel))
        });
        let ids: HashSet<usize> = first.iter().map(|h| h.0).collect();
        first.extend(results.into_iter().filter(|h| !ids.contains(&h.0)));
        results = first;
    }
    Some((q, docs, results))
}

/// The date range a time expression in the terms or the question names, if any.
fn period(terms: &[String], question: Option<&str>) -> Option<crate::when::Range> {
    crate::when::range(&format!("{} {}", terms.join(" "), question.unwrap_or("")), util::today())
}

/// Each section's text for the vector index: (doc index, section index, "Title. Heading\nbody").
/// Private pages are left out.
fn section_texts(docs: &[Doc]) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    for (i, d) in docs.iter().enumerate() {
        if d.private {
            continue;
        }
        let title = if d.title.is_empty() {
            d.rel.rsplit('/').next().unwrap_or(&d.rel).trim_end_matches(".md").to_string()
        } else {
            d.title.clone()
        };
        for (k, p) in d.parts.iter().enumerate() {
            let from = p.line.map(|l| l + 1).unwrap_or(0);
            let to = d.parts.get(k + 1).and_then(|n| n.line).unwrap_or(d.body.len());
            let body: Vec<&str> = d.body[from.min(to)..to]
                .iter()
                .map(String::as_str)
                .filter(|l| !l.trim().is_empty() && !l.starts_with("# ") && !l.starts_with("Topics:"))
                .collect();
            if body.is_empty() && p.heading.is_empty() {
                continue;
            }
            let text: String = format!("{title}. {}\n{}", p.heading, body.join("\n")).chars().take(2000).collect();
            out.push((i, k, text));
        }
    }
    out
}

/// `kioku semantic index`: embed every section not in the cache yet.
pub fn index_all(vault: &Path) -> anyhow::Result<usize> {
    let mut files = Vec::new();
    for d in ["Core.md", "projects", "wiki", "daily", "journal", "calendar", "contacts", "reviews", "index.md"] {
        walk(&vault.join(d), &mut files);
    }
    let docs: Vec<Doc> = files.iter().map(|f| parse(vault, f)).collect();
    crate::semantic::index(vault, &section_texts(&docs))
}

/// For each text, the vault page whose section is most like it by meaning, and the cosine
/// similarity (hybrid search's model; None when unavailable).
pub fn semantic_matches(vault: &Path, texts: &[String]) -> Option<Vec<(String, f32)>> {
    let mut files = Vec::new();
    for d in ["Core.md", "AGENTS.md", "projects", "wiki", "daily", "journal", "reviews", "index.md"] {
        walk(&vault.join(d), &mut files);
    }
    let docs: Vec<Doc> = files.iter().map(|f| parse(vault, f)).collect();
    let r = crate::semantic::best(vault, &section_texts(&docs), texts)?;
    Some(r.into_iter().map(|(d, s)| (docs[d].rel.clone(), s)).collect())
}

/// The best pages for the terms: (relative path, title), private pages left out.
pub fn top(vault: &Path, terms: &[String], n: usize) -> Vec<(String, String)> {
    top_q(vault, terms, None, n)
}
/// Like `top`, with the user's question for the vector side of hybrid search.
pub fn top_q(vault: &Path, terms: &[String], question: Option<&str>, n: usize) -> Vec<(String, String)> {
    let Some((_, docs, results)) = rank(vault, terms, false, question) else {
        return vec![];
    };
    results
        .into_iter()
        .filter(|r| !docs[r.0].private)
        .take(n)
        .map(|r| (docs[r.0].rel.clone(), docs[r.0].title.clone()))
        .collect()
}

pub fn run(vault: &Path, args: &[String]) -> anyhow::Result<i32> {
    match render(vault, args) {
        Some(out) => {
            print!("{out}");
            Ok(0)
        }
        None => {
            eprintln!("kioku: usage: kioku search [--raw] [-n N] [-q \"question\"] <term...>");
            Ok(2)
        }
    }
}

/// The search output as text; None when no term was given.
pub fn render(vault: &Path, args: &[String]) -> Option<String> {
    let mut raw = false;
    let mut limit = 20usize;
    let mut terms: Vec<String> = Vec::new();
    let mut question: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--raw" => raw = true,
            "-n" | "--limit" => limit = it.next().and_then(|v| v.parse::<usize>().ok()).filter(|n| *n > 0).unwrap_or(20),
            "-q" | "--question" => question = it.next().cloned(),
            _ => terms.push(a.clone()),
        }
    }
    if terms.is_empty() {
        return None;
    }

    let mut out = String::new();
    let Some((q, docs, results)) = rank(vault, &terms, raw, question.as_deref()) else {
        return Some(out);
    };
    if let Some((a, b)) = period(&terms, question.as_deref()) {
        out.push_str(&format!("(time: {a} … {b}; pages from that period first)\n"));
    }

    let line_hits = |l: &str| {
        let f = fold(l);
        q.iter().any(|t| match t {
            Term::Phrase(p) => f.contains(p.as_str()),
            Term::Word(w) => word_re().find_iter(&f).any(|m| word_match(w, m.as_str()) > 0.0),
        })
    };
    for (i, s, matched, at) in results.into_iter().take(limit) {
        let d = &docs[i];
        let p = if d.private { " [private]" } else { "" };
        let m = if q.len() > 1 {
            format!(" {matched}/{}", q.len())
        } else {
            String::new()
        };
        let title = if d.title.is_empty() { "?" } else { &d.title };
        let p = match &d.successor {
            Some(s) => format!("{p} [superseded by [[{s}]]]"),
            None => p.to_string(),
        };
        out.push_str(&format!("{} — {}{}  ({:.2}{})\n", d.rel, title, p, s, m));
        // The best section first (its heading, then its matching lines), then the rest.
        let (from, to) = match d.parts.get(at).and_then(|p| p.line) {
            Some(l) if d.parts.len() > 1 => {
                let end = d.parts.get(at + 1).and_then(|p| p.line).unwrap_or(d.body.len());
                out.push_str(&format!("    {}:§ {}\n", l + 1 + d.body_start, d.parts[at].heading));
                (l + 1, end)
            }
            _ => (0, 0),
        };
        let order = (from..to).chain((0..d.body.len()).filter(|n| *n < from || *n >= to));
        let mut shown = 0;
        for n in order {
            let l = &d.body[n];
            if shown >= 2 {
                break;
            }
            if l.trim().is_empty() || l.starts_with("# ") || !line_hits(l) {
                continue;
            }
            out.push_str(&format!("    {}:{}\n", n + 1 + d.body_start, util::take(l.trim(), 200)));
            shown += 1;
        }
    }
    Some(out)
}
