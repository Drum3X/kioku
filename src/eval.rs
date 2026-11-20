//! `kioku eval [-v] [FILE.tsv ...]`: how well `kioku search` finds the right page for real
//! questions. Each line of a TSV file is `question<TAB>terms;separated;by;semicolons<TAB>
//! expected/page.md|or/another.md`. Every question is searched twice: with its terms (what an
//! agent passes) and with the whole question. An empty expected column means the vault has
//! nothing on it: the search should come back empty. Default files: tests/queries/*.tsv in the
//! Kioku checkout.

use crate::{search, util};
use anyhow::Result;
use std::path::{Path, PathBuf};

const DEPTH: usize = 10;

struct Case {
    file: String,
    line: usize,
    question: String,
    terms: Vec<String>,
    expected: Vec<String>,
}

fn load(f: &Path) -> Vec<Case> {
    let name = f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    util::read(f)
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|(i, l)| {
            let mut c = l.split('\t');
            let question = c.next().unwrap_or("").trim().to_string();
            let terms = c
                .next()
                .unwrap_or("")
                .split(';')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            let expected = c
                .next()
                .unwrap_or("")
                .split('|')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            Case { file: name.clone(), line: i + 1, question, terms, expected }
        })
        .collect()
}

/// The 1-based rank of the first expected page in the results, if any.
fn rank(hits: &[(String, String)], expected: &[String]) -> Option<usize> {
    hits.iter().position(|(r, _)| expected.contains(r)).map(|p| p + 1)
}

#[derive(Default)]
struct Score {
    n: usize,
    at1: usize,
    at3: usize,
    at10: usize,
    rr: f64,
}
impl Score {
    fn add(&mut self, r: Option<usize>) {
        self.n += 1;
        if let Some(r) = r {
            self.at1 += (r == 1) as usize;
            self.at3 += (r <= 3) as usize;
            self.at10 += (r <= DEPTH) as usize;
            self.rr += 1.0 / r as f64;
        }
    }
    fn row(&self, label: &str) -> String {
        let mrr = if self.n == 0 { 0.0 } else { self.rr / self.n as f64 };
        format!(
            "{label:<22} {:>3}/{:<3} {:>3}/{:<3} {:>3}/{:<3}  {mrr:.2}",
            self.at1, self.n, self.at3, self.n, self.at10, self.n
        )
    }
}

pub fn run(vault: &Path, args: &[String]) -> Result<i32> {
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
    let mut files: Vec<PathBuf> = args.iter().filter(|a| !a.starts_with('-')).map(PathBuf::from).collect();
    if files.is_empty() {
        let dir = util::sys_dir().join("tests/queries");
        files = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "tsv")).collect())
            .unwrap_or_default();
        files.sort();
        if files.is_empty() {
            return util::die(format!("no question files in {}; usage: kioku eval [-v] [FILE.tsv ...]", dir.display()));
        }
    }
    let cases: Vec<Case> = files.iter().flat_map(|f| load(f)).collect();
    if cases.is_empty() {
        return util::die("no questions in the given files");
    }

    let mut by_terms = Score::default();
    let mut by_question = Score::default();
    let mut per_file: Vec<(String, Score, Score)> = Vec::new();
    let (mut abstain, mut abstain_terms, mut abstain_question) = (0, 0, 0);
    let mut misses = Vec::new();
    let mut detail = Vec::new();
    for c in &cases {
        let words: Vec<String> = c.question.split_whitespace().map(str::to_string).collect();
        let ht = if c.terms.is_empty() { vec![] } else { search::top(vault, &c.terms, DEPTH) };
        let hq = search::top(vault, &words, DEPTH);
        if c.expected.is_empty() {
            abstain += 1;
            abstain_terms += ht.is_empty() as usize;
            abstain_question += hq.is_empty() as usize;
            let top = |h: &[(String, String)]| h.first().map(|r| r.0.clone()).unwrap_or_else(|| "nothing".into());
            let l = format!(
                "- {}:{} \"{}\": should find nothing; terms → {}, question → {}",
                c.file, c.line, c.question, top(&ht), top(&hq)
            );
            if verbose {
                detail.push(l);
            } else if !ht.is_empty() || !hq.is_empty() {
                misses.push(l);
            }
            continue;
        }
        let (rt, rq) = (rank(&ht, &c.expected), rank(&hq, &c.expected));
        by_terms.add(rt);
        by_question.add(rq);
        let pf = match per_file.iter().position(|p| p.0 == c.file) {
            Some(i) => &mut per_file[i],
            None => {
                per_file.push((c.file.clone(), Score::default(), Score::default()));
                per_file.last_mut().unwrap()
            }
        };
        pf.1.add(rt);
        pf.2.add(rq);
        let show = |r: Option<usize>| r.map(|r| r.to_string()).unwrap_or_else(|| format!(">{DEPTH}"));
        let l = format!(
            "- {}:{} \"{}\": terms {}, question {}{}",
            c.file,
            c.line,
            c.question,
            show(rt),
            show(rq),
            if rt.is_none_or(|r| r > 3) || rq.is_none_or(|r| r > 3) {
                format!(" (expected {}; got {})", c.expected.join(" | "), hq.first().map(|h| h.0.as_str()).unwrap_or("nothing"))
            } else {
                String::new()
            }
        );
        if verbose {
            detail.push(l);
        } else if rt.is_none_or(|r| r > 3) || rq.is_none_or(|r| r > 3) {
            misses.push(l);
        }
    }

    println!(
        "Kioku eval: {} question(s) from {}; vault {}",
        cases.len(),
        files.iter().map(|f| f.file_name().unwrap_or_default().to_string_lossy().into_owned()).collect::<Vec<_>>().join(", "),
        vault.display()
    );
    println!("Rank of the first right page (top 1 / top 3 / top {DEPTH}, mean reciprocal rank):");
    println!("{:<22} {:>7} {:>7} {:>7}  MRR", "", "top 1", "top 3", "top 10");
    println!("{}", by_terms.row("terms (agent's words)"));
    println!("{}", by_question.row("whole question"));
    if per_file.len() > 1 {
        for (f, t, q) in &per_file {
            println!("{}", t.row(&format!("  {f}, terms")));
            println!("{}", q.row(&format!("  {f}, question")));
        }
    }
    if abstain > 0 {
        println!("Should find nothing: {abstain_terms}/{abstain} with terms, {abstain_question}/{abstain} with the whole question");
    }
    if verbose {
        println!("\nAll questions (rank by terms, by whole question):");
        for l in &detail {
            println!("{l}");
        }
    } else if !misses.is_empty() {
        println!("\nNot in the top 3 (rank by terms, by whole question):");
        for l in &misses {
            println!("{l}");
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ranks_and_scores() {
        let hits = vec![("a.md".to_string(), String::new()), ("b.md".to_string(), String::new())];
        assert_eq!(rank(&hits, &["b.md".into()]), Some(2));
        assert_eq!(rank(&hits, &["c.md".into()]), None);
        let mut s = Score::default();
        s.add(Some(1));
        s.add(Some(2));
        s.add(None);
        assert_eq!((s.n, s.at1, s.at3, s.at10), (3, 1, 2, 2));
        assert!((s.rr - 1.5).abs() < 1e-9);
    }
}
