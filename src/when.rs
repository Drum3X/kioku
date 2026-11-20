//! Time expressions in search questions ("geçen hafta", "dün", "2 hafta sonra", "last month"):
//! the date range they name, the words that make them up (left out of the BM25 terms), and the
//! dates a page covers (daily and journal days, review weeks and months, calendar events).

use crate::text::fold;
use chrono::{Datelike, Duration, NaiveDate, Weekday};
use regex::Regex;
use std::sync::OnceLock;

pub type Range = (NaiveDate, NaiveDate);

const NUMS: [(&str, i64); 12] = [
    ("bir", 1), ("iki", 2), ("uc", 3), ("dort", 4), ("bes", 5), ("alti", 6), ("yedi", 7),
    ("one", 1), ("two", 2), ("three", 3), ("four", 4), ("five", 5),
];
fn num(s: &str) -> i64 {
    s.parse().ok().or_else(|| NUMS.iter().find(|n| n.0 == s).map(|n| n.1)).unwrap_or(1)
}
fn monday(d: NaiveDate) -> NaiveDate {
    d - Duration::days(d.weekday().num_days_from_monday() as i64)
}
fn month_start(d: NaiveDate, back: i32) -> NaiveDate {
    let (mut y, mut m) = (d.year(), d.month() as i32 - back);
    while m < 1 {
        m += 12;
        y -= 1;
    }
    NaiveDate::from_ymd_opt(y, m as u32, 1).unwrap()
}

/// Words that only carry the time: dropped from the search terms when a range was found.
pub fn is_time_word(w: &str) -> bool {
    const W: &str = " bugun bugunku dun dunku evvelsi onceki yarin gecen gecenki gectigimiz bu hafta haftaki haftada haftalar ay ayki ayda son once sonra sonraki gelecek civari civarinda kadar today yesterday tomorrow last this next week weeks month months ago in days day gun gunluk ";
    let w = fold(w);
    W.contains(&format!(" {w} ")) || w.chars().all(|c| c.is_ascii_digit()) || NUMS.iter().any(|n| n.0 == w)
}

/// The date range a question names, relative to `today`. Past periods run up to today, so
/// "geçen hafta" covers last week and this week so far (both readings people mean).
pub fn range(text: &str, today: NaiveDate) -> Option<Range> {
    static R: OnceLock<Vec<(Regex, u8)>> = OnceLock::new();
    let n = r"(\d+|bir|iki|uc|dort|bes|alti|yedi|one|two|three|four|five)";
    let rs = R.get_or_init(|| {
        [
            (format!(r"\bson {n} gun|\blast {n} days"), 1),
            (format!(r"\b{n} gun (once|evvel)|\b{n} days? ago"), 2),
            (format!(r"\b{n} hafta (once|evvel)|\b{n} weeks? ago"), 3),
            (format!(r"\b{n} ay (once|evvel)|\b{n} months? ago"), 4),
            (format!(r"\b{n} gun sonra|\bin {n} days"), 5),
            (format!(r"\b{n} hafta sonra|\bin {n} weeks?"), 6),
            (format!(r"\b{n} ay\b.{{0,20}}sonra|\bin {n} months?"), 7),
            (r"\b(gecen|gectigimiz|onceki) hafta|\blast week".into(), 8),
            (r"\bbu hafta|\bthis week".into(), 9),
            (r"\b(gecen|gectigimiz|onceki) ay|\blast month".into(), 10),
            (r"\bbu ay\b|\bthis month".into(), 11),
            (r"\b(gelecek|onumuzdeki) hafta|\bnext week".into(), 12),
            (r"\b(evvelsi|onceki) gun|\bday before yesterday".into(), 13),
            (r"\bdun\b|\bdunku|\byesterday".into(), 14),
            (r"\bbugun|\btoday".into(), 15),
            (r"\byarin|\btomorrow".into(), 16),
        ]
        .into_iter()
        .map(|(p, k)| (Regex::new(&p).unwrap(), k))
        .collect()
    });
    let t = fold(text);
    let d = |n: i64| today + Duration::days(n);
    for (re, kind) in rs {
        let Some(c) = re.captures(&t) else { continue };
        let k = c.iter().skip(1).flatten().map(|m| num(m.as_str())).next().unwrap_or(1);
        return Some(match kind {
            1 => (d(-k), today),
            2 => (d(-k - 1), d(-k + 1)),
            3 => (d(-7 * k - 3), d(-7 * k + 3)),
            4 => (d(-30 * k - 10), d(-30 * k + 10)),
            5 => (d(k - 1), d(k + 1)),
            6 => (d(7 * k - 3), d(7 * k + 3)),
            7 => (d(30 * k - 15), d(30 * k + 15)),
            8 => (monday(today) - Duration::days(7), today),
            9 => (monday(today), today),
            10 => (month_start(today, 1), today),
            11 => (month_start(today, 0), today),
            12 => (monday(today) + Duration::days(7), monday(today) + Duration::days(13)),
            13 => (d(-2), d(-2)),
            14 => (d(-1), d(-1)),
            15 => (today, today),
            _ => (d(1), d(1)),
        });
    }
    None
}

/// The dates a page covers, from its path: daily/ and journal/ days, calendar events (the date
/// in the file name), reviews (week-YYYY-wNN, month-YYYY-MM).
pub fn span(rel: &str) -> Option<Range> {
    let name = rel.rsplit('/').next()?.trim_end_matches(".md");
    let day = |s: &str| NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok();
    if rel.starts_with("daily/") || rel.starts_with("journal/") || rel.starts_with("calendar/") {
        let d = day(name)?;
        return Some((d, d));
    }
    if rel.starts_with("reviews/") {
        if let Some(w) = name.strip_prefix("week-") {
            let (y, n) = w.split_once("-w")?;
            let mon = NaiveDate::from_isoywd_opt(y.parse().ok()?, n.parse().ok()?, Weekday::Mon)?;
            return Some((mon, mon + Duration::days(6)));
        }
        if let Some(m) = name.strip_prefix("month-") {
            let first = NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").ok()?;
            let next = month_start(first + Duration::days(32), 0);
            return Some((first, next - Duration::days(1)));
        }
    }
    None
}

pub fn overlaps(a: Range, b: Range) -> bool {
    a.0 <= b.1 && b.0 <= a.1
}

#[cfg(test)]
mod tests {
    use super::*;
    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    #[test]
    fn ranges() {
        let t = d("2026-11-20"); // a Friday
        assert_eq!(range("geçen hafta yaptığımız işler", t), Some((d("2026-11-09"), t)));
        assert_eq!(range("dün ne yaptık", t), Some((d("2026-11-19"), d("2026-11-19"))));
        assert_eq!(range("2 hafta sonraki toplantım", t), Some((d("2026-12-01"), d("2026-12-07"))));
        assert_eq!(range("iki ay civarı bir süre sonra", t), Some((d("2027-01-04"), d("2027-02-03"))));
        assert_eq!(range("what did we do last month", t), Some((d("2026-10-01"), t)));
        assert_eq!(range("disk adları neden değişiyor", t), None);
        assert_eq!(range("bu ayarı nasıl yaptık", t), None);
    }
    #[test]
    fn spans() {
        assert_eq!(span("daily/2026-11-19.md"), Some((d("2026-11-19"), d("2026-11-19"))));
        assert_eq!(span("reviews/2026/week-2026-w47.md"), Some((d("2026-11-16"), d("2026-11-22"))));
        assert_eq!(span("calendar/2026/2026-12-05-kardesimin-dogum-gunu.md"), Some((d("2026-12-05"), d("2026-12-05"))));
        assert_eq!(span("wiki/entities/krohnkite.md"), None);
        assert!(is_time_word("geçen") && is_time_word("hafta") && !is_time_word("disk"));
    }
}
