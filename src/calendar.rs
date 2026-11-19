//! Events (calendar/), todos (journal/) and contacts (contacts/): the agenda and offline checks.

use crate::fm::{self, Val};
use crate::util;
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime, Timelike};
use regex::Regex;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Local "YYYY-MM-DD" or "YYYY-MM-DD[T ]HH:MM" → (datetime, has time). Dates that do not exist
/// (2026-11-31, 25:00) are invalid.
pub fn parse_local(s: &str) -> Option<(NaiveDateTime, bool)> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{1,2}):(\d{2}))?$").unwrap());
    let c = r.captures(s.trim())?;
    let n = |i: usize| c.get(i).map(|m| m.as_str().parse::<u32>().unwrap_or(99));
    let d = NaiveDate::from_ymd_opt(n(1)? as i32, n(2)?, n(3)?)?;
    let has_time = c.get(4).is_some();
    let dt = d.and_hms_opt(n(4).unwrap_or(0), n(5).unwrap_or(0), 0)?;
    Some((dt, has_time))
}

/// JavaScript's setMonth: the day of month is kept and overflows into the next month.
fn add_months_js(dt: NaiveDateTime, k: i32) -> NaiveDateTime {
    let total = dt.year() * 12 + dt.month0() as i32 + k;
    let (y, m0) = (total.div_euclid(12), total.rem_euclid(12) as u32);
    let first = NaiveDate::from_ymd_opt(y, m0 + 1, 1).unwrap();
    (first + Duration::days(dt.day() as i64 - 1)).and_time(dt.time())
}

pub struct Event {
    pub file: PathBuf,
    pub data: BTreeMap<String, Val>,
    pub start: NaiveDateTime,
    pub all_day: bool,
    pub end: NaiveDateTime,
    pub title: String,
}

pub fn read_events(vault: &Path, problems: &mut Vec<String>) -> Vec<Event> {
    static DUR: OnceLock<Regex> = OnceLock::new();
    let dur = DUR.get_or_init(|| Regex::new(r"^(\d+)\s*(m|h)$").unwrap());
    static RR: OnceLock<Regex> = OnceLock::new();
    let rr_re = RR.get_or_init(|| Regex::new(r"^FREQ=(DAILY|WEEKLY|MONTHLY|YEARLY)").unwrap());
    static AL: OnceLock<Regex> = OnceLock::new();
    let al_re = AL.get_or_init(|| Regex::new(r"^\d+\s*[mhd]$").unwrap());
    let mut out = Vec::new();
    for f in util::md_files(&vault.join("calendar")) {
        let (data, _) = fm::parse(&util::read(&f));
        let ty = fm::s(&data, "type");
        if !ty.is_empty() && ty != "event" {
            continue;
        }
        let rel = util::rel(vault, &f);
        let Some((start, has_time)) = parse_local(&fm::s(&data, "start")) else {
            problems.push(format!("{rel}: invalid or missing start (YYYY-MM-DD or YYYY-MM-DDTHH:MM)"));
            continue;
        };
        let all_day = fm::s(&data, "all_day") == "true" || !has_time;
        let end_s = fm::s(&data, "end");
        let end = if end_s.is_empty() { None } else { parse_local(&end_s) };
        if !end_s.is_empty() && end.is_none() {
            problems.push(format!("{rel}: invalid end"));
        }
        // All-day: the day after the last day (JavaScript's addDays keeps the time of day).
        let end_d = if all_day {
            end.map(|e| e.0).unwrap_or(start) + Duration::days(1)
        } else if let Some((e, true)) = end {
            e
        } else {
            let ds = fm::s(&data, "duration");
            let m = dur.captures(&ds);
            if !ds.is_empty() && m.is_none() {
                problems.push(format!("{rel}: duration must look like 30m or 2h"));
            }
            let mins = m
                .map(|c| c[1].parse::<i64>().unwrap_or(0) * if &c[2] == "h" { 60 } else { 1 })
                .unwrap_or(60);
            start + Duration::minutes(mins)
        };
        if end_d <= start {
            problems.push(format!("{rel}: end is before start"));
        }
        let rr = fm::s(&data, "rrule");
        if !rr.is_empty() && !rr_re.is_match(&rr) {
            problems.push(format!("{rel}: rrule must look like FREQ=WEEKLY;BYDAY=MO"));
        }
        let al = fm::s(&data, "alarm");
        if !al.is_empty() && !al_re.is_match(&al) {
            problems.push(format!("{rel}: alarm must look like 30m, 2h or 1d"));
        }
        let title = Some(fm::s(&data, "title"))
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        out.push(Event {
            file: f,
            data,
            start,
            all_day,
            end: end_d,
            title,
        });
    }
    out
}

fn occurrences(ev: &Event, from: NaiveDateTime, to: NaiveDateTime) -> Vec<NaiveDateTime> {
    let start = ev.start;
    let span = ev.end - start;
    let rrule = fm::s(&ev.data, "rrule");
    if rrule.is_empty() {
        return if start < to && start + span > from { vec![start] } else { vec![] };
    }
    let r: HashMap<String, String> = rrule
        .split(';')
        .map(|p| {
            let mut it = p.splitn(2, '=');
            (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
        })
        .collect();
    let num = |k: &str| r.get(k).and_then(|v| v.parse::<i64>().ok()).filter(|n| *n != 0);
    let interval = num("INTERVAL").unwrap_or(1);
    let count = num("COUNT").map(|c| c as f64).unwrap_or(f64::INFINITY);
    let until = r.get("UNTIL").and_then(|u| {
        let d = u.get(..8)?;
        NaiveDate::parse_from_str(d, "%Y%m%d").ok()?.and_hms_opt(23, 59, 0)
    });
    let mut out = Vec::new();
    let mut n = 0f64;
    let mut take = |d: NaiveDateTime, out: &mut Vec<NaiveDateTime>| -> bool {
        if until.is_some_and(|u| d > u) {
            return false;
        }
        n += 1.0;
        if n > count {
            return false;
        }
        if d >= from && d < to {
            out.push(d);
        }
        d < to
    };
    let freq = r.get("FREQ").map(String::as_str).unwrap_or("");
    if freq == "WEEKLY" && r.get("BYDAY").is_some_and(|v| !v.is_empty()) {
        let wd = |s: &str| match s {
            "SU" => Some(0),
            "MO" => Some(1),
            "TU" => Some(2),
            "WE" => Some(3),
            "TH" => Some(4),
            "FR" => Some(5),
            "SA" => Some(6),
            _ => None,
        };
        let mut days: Vec<i64> = r["BYDAY"]
            .split(',')
            .filter_map(|x| {
                let t: String = x.chars().rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
                wd(&t)
            })
            .collect();
        days.sort();
        let week0 = start - Duration::days(start.weekday().num_days_from_sunday() as i64);
        let mut w = 0i64;
        while w < 3000 {
            for &d0 in &days {
                let d = (week0 + Duration::days(w * 7 + d0))
                    .date()
                    .and_hms_opt(start.hour(), start.minute(), 0)
                    .unwrap();
                if d < start {
                    continue;
                }
                if !take(d, &mut out) {
                    return out;
                }
            }
            w += interval;
        }
        return out;
    }
    let mut d = start;
    for _ in 0..20000 {
        if !take(d, &mut out) {
            break;
        }
        d = match freq {
            "DAILY" => d + Duration::days(interval),
            "WEEKLY" => d + Duration::days(7 * interval),
            "MONTHLY" => add_months_js(d, interval as i32),
            "YEARLY" => add_months_js(d, 12 * interval as i32),
            _ => break,
        };
    }
    out
}

fn cancelled(ev: &Event) -> bool {
    fm::s(&ev.data, "status").to_lowercase() == "cancelled"
}

/// `kioku agenda [--days N] [--past]` lines (or the "no events" line).
pub fn agenda(vault: &Path, days: i64, past: bool) -> String {
    let now = Local::now().naive_local();
    let from = now.date().and_hms_opt(0, 0, 0).unwrap();
    let to = from + Duration::days(days);
    let mut rows: Vec<(NaiveDateTime, String)> = Vec::new();
    for ev in read_events(vault, &mut Vec::new()) {
        if cancelled(&ev) {
            continue;
        }
        let span = ev.end - ev.start;
        for d in occurrences(&ev, from, to) {
            let end = d + span;
            if end <= now && !past {
                continue;
            }
            let when = if end <= now {
                " (ended)"
            } else if d <= now {
                " (now)"
            } else {
                ""
            };
            let time = if ev.all_day {
                "all day".to_string()
            } else {
                d.format("%H:%M").to_string()
            };
            let loc = fm::s(&ev.data, "location");
            let loc = if loc.is_empty() { String::new() } else { format!(" ({loc})") };
            let rec = if fm::s(&ev.data, "rrule").is_empty() { "" } else { " [recurring]" };
            rows.push((
                d,
                format!(
                    "- {} {}{} {}{}{} — {}",
                    d.format("%a %Y-%m-%d"),
                    time,
                    when,
                    ev.title,
                    loc,
                    rec,
                    util::rel(vault, &ev.file)
                ),
            ));
        }
    }
    rows.sort_by_key(|r| r.0);
    if rows.is_empty() {
        return format!("(no events in the next {days} days)\n");
    }
    rows.into_iter().map(|r| r.1 + "\n").collect()
}

pub fn cmd_agenda(vault: &Path, args: &[String]) -> i32 {
    let mut days = 7;
    if let Some(i) = args.iter().position(|a| a == "--days") {
        days = args.get(i + 1).and_then(|v| v.parse::<i64>().ok()).filter(|n| *n >= 1).unwrap_or(7);
    }
    print!("{}", agenda(vault, days, args.iter().any(|a| a == "--past")));
    0
}

/// Offline checks of todos, events and contacts: "- problem" lines.
pub fn validate(vault: &Path) -> Vec<String> {
    let mut p = Vec::new();
    static TODO: OnceLock<Regex> = OnceLock::new();
    static REM: OnceLock<Regex> = OnceLock::new();
    let todo = TODO.get_or_init(|| Regex::new(r"^(\s*- \[)([ xX-])(\] )(.*)$").unwrap());
    let rem = REM.get_or_init(|| Regex::new(r"^(\d+\s*[mhd]|0|now)$").unwrap());
    let grab = |text: &str, k: &str| -> Option<String> {
        let i = text.find(&format!("({k}:"))?;
        let rest = &text[i + k.len() + 2..];
        let j = rest.find(')')?;
        Some(rest[..j].trim_start().to_string())
    };
    for f in util::md_files(&vault.join("journal")) {
        let rel = util::rel(vault, &f);
        for (i, line) in util::read(&f).split('\n').enumerate() {
            let Some(c) = todo.captures(line) else { continue };
            let text = &c[4];
            let due = grab(text, "due");
            let at = grab(text, "at").filter(|a| !a.is_empty());
            if let Some(d) = &due {
                let s = match &at {
                    Some(a) => format!("{}T{}", d.trim(), a.trim()),
                    None => d.clone(),
                };
                if parse_local(&s).is_none() {
                    p.push(format!("{rel}:{}: invalid due date or time", i + 1));
                }
            }
            if let Some(r) = grab(text, "remind") {
                if !rem.is_match(r.trim()) {
                    p.push(format!("{rel}:{}: remind must look like 30m, 2h, 1d or 0", i + 1));
                } else if due.is_none() {
                    p.push(format!("{rel}:{}: remind needs a due date", i + 1));
                }
            }
        }
    }
    read_events(vault, &mut p);
    for f in util::md_files(&vault.join("contacts")) {
        let (data, _) = fm::parse(&util::read(&f));
        let ty = fm::s(&data, "type");
        if !ty.is_empty() && ty != "contact" {
            continue;
        }
        let b = fm::s(&data, "birthday");
        if !b.is_empty() && parse_local(&b).is_none() {
            p.push(format!("{}: birthday must be YYYY-MM-DD", util::rel(vault, &f)));
        }
    }
    p.into_iter().map(|x| format!("- {x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_dates() {
        assert!(parse_local("2026-11-30").is_some());
        assert!(parse_local("2026-11-31").is_none());
        assert!(parse_local("2026-13-01").is_none());
        assert!(parse_local("2026-11-20T25:00").is_none());
        assert_eq!(parse_local(" 2026-11-20 9:05").map(|x| x.1), Some(true));
    }
    #[test]
    fn js_months() {
        let d = parse_local("2026-01-31T10:00").unwrap().0;
        assert_eq!(add_months_js(d, 1).format("%F %R").to_string(), "2026-03-03 10:00");
        let l = parse_local("2028-02-29").unwrap().0;
        assert_eq!(add_months_js(l, 12).format("%F").to_string(), "2029-03-01");
    }
}
