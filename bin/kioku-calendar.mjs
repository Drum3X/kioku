#!/usr/bin/env node
// kioku calendar engine (no dependencies, Node 18+). Called through `kioku agenda|lint-check|check-file`.
//
//   agenda [--days N] [--past]   upcoming events from calendar/ (recurrences expanded; ended ones only with --past)
//   validate                     offline checks of todos (journal/), events (calendar/) and contacts (contacts/)
//
// Everything stays in the vault; nothing is sent anywhere.

import fs from 'node:fs';
import path from 'node:path';

const VAULT = process.env.KIOKU_DIR || path.resolve(path.dirname(process.argv[1]), '../..');

// ---------- small helpers ----------
const pad = (n) => String(n).padStart(2, '0');
const rel = (f) => path.relative(VAULT, f);
const isoDay = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const addDays = (d, n) => { const x = new Date(d); x.setDate(x.getDate() + n); return x; };

function mdFiles(dir) {
  const out = [];
  const walk = (d) => {
    if (!fs.existsSync(d)) return;
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.md')) out.push(p);
    }
  };
  walk(dir);
  return out.sort();
}

const unq = (s) => s.replace(/^"(.*)"$/, '$1').replace(/^'(.*)'$/, '$1');
function parseFrontmatter(text) {
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!m) return { data: {}, body: text };
  const data = {};
  let key = null;
  for (const line of m[1].split(/\r?\n/)) {
    const li = line.match(/^\s+-\s+(.*)$/);
    if (li && key) {
      if (!Array.isArray(data[key])) data[key] = [];
      data[key].push(unq(li[1].trim()));
      continue;
    }
    const kv = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/);
    if (!kv) continue;
    key = kv[1];
    let v = kv[2].trim();
    if (!/^["']/.test(v)) v = v.replace(/\s+#.*$/, '');
    if (v.startsWith('[') && v.endsWith(']')) data[key] = v.slice(1, -1).split(',').map((s) => unq(s.trim())).filter(Boolean);
    else data[key] = unq(v);
  }
  return { data, body: text.slice(m[0].length) };
}

// Local "YYYY-MM-DD" or "YYYY-MM-DD[T ]HH:MM" → { date, time, d }
function parseLocal(s) {
  const m = String(s || '').trim().match(/^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{1,2}):(\d{2}))?$/);
  if (!m) return null;
  const d = new Date(+m[1], +m[2] - 1, +m[3], +(m[4] || 0), +(m[5] || 0));
  // Date rolls 2026-11-31 over to December 1st; such dates are invalid, not moved.
  if (isNaN(d) || d.getMonth() !== +m[2] - 1 || d.getDate() !== +m[3] || +(m[4] || 0) > 23 || +(m[5] || 0) > 59) return null;
  return { date: `${m[1]}-${m[2]}-${m[3]}`, time: m[4] ? `${pad(m[4])}:${m[5]}` : null, d };
}

// ---------- read the vault ----------
const TODO_RE = /^(\s*- \[)([ xX-])(\] )(.*)$/;

function checkTodos(problems) {
  for (const f of mdFiles(path.join(VAULT, 'journal'))) {
    fs.readFileSync(f, 'utf8').split('\n').forEach((line, i) => {
      const m = line.match(TODO_RE);
      if (!m) return;
      const text = m[4];
      const due = (text.match(/\(due:\s*([^)]*)\)/) || [])[1];
      const at = (text.match(/\(at:\s*([^)]*)\)/) || [])[1];
      const remind = (text.match(/\(remind:\s*([^)]*)\)/) || [])[1];
      if (due !== undefined && !parseLocal(at ? `${due.trim()}T${at.trim()}` : due)) problems.push(`${rel(f)}:${i + 1}: invalid due date or time`);
      if (remind !== undefined) {
        if (!/^(\d+\s*[mhd]|0|now)$/.test(remind.trim())) problems.push(`${rel(f)}:${i + 1}: remind must look like 30m, 2h, 1d or 0`);
        else if (due === undefined) problems.push(`${rel(f)}:${i + 1}: remind needs a due date`);
      }
    });
  }
}

function readEvents(problems) {
  const events = [];
  for (const f of mdFiles(path.join(VAULT, 'calendar'))) {
    const { data, body } = parseFrontmatter(fs.readFileSync(f, 'utf8'));
    if (data.type && data.type !== 'event') continue;
    const start = parseLocal(data.start);
    if (!start) { problems.push(`${rel(f)}: invalid or missing start (YYYY-MM-DD or YYYY-MM-DDTHH:MM)`); continue; }
    const allDay = String(data.all_day) === 'true' || !start.time;
    const end = data.end ? parseLocal(data.end) : null;
    if (data.end && !end) problems.push(`${rel(f)}: invalid end`);
    let endD;
    if (allDay) endD = addDays(end ? end.d : start.d, 1);               // the day after the last day
    else if (end && end.time) endD = end.d;
    else {
      const dm = String(data.duration || '').match(/^(\d+)\s*(m|h)$/);
      if (data.duration && !dm) problems.push(`${rel(f)}: duration must look like 30m or 2h`);
      endD = new Date(start.d.getTime() + (dm ? +dm[1] * (dm[2] === 'h' ? 3600e3 : 60e3) : 3600e3));
    }
    if (endD <= start.d) problems.push(`${rel(f)}: end is before start`);
    if (data.rrule && !/^FREQ=(DAILY|WEEKLY|MONTHLY|YEARLY)/.test(data.rrule)) problems.push(`${rel(f)}: rrule must look like FREQ=WEEKLY;BYDAY=MO`);
    if (data.alarm && !/^\d+\s*[mhd]$/.test(String(data.alarm))) problems.push(`${rel(f)}: alarm must look like 30m, 2h or 1d`);
    events.push({ file: f, data, body, start, allDay, endD, title: data.title || path.basename(f, '.md') });
  }
  return events;
}

function checkContacts(problems) {
  for (const f of mdFiles(path.join(VAULT, 'contacts'))) {
    const { data } = parseFrontmatter(fs.readFileSync(f, 'utf8'));
    if (data.type && data.type !== 'contact') continue;
    if (data.birthday && !parseLocal(data.birthday)) problems.push(`${rel(f)}: birthday must be YYYY-MM-DD`);
  }
}

// ---------- agenda ----------
const WD = { SU: 0, MO: 1, TU: 2, WE: 3, TH: 4, FR: 5, SA: 6 };
function occurrences(ev, from, to) {
  const start = ev.start.d;
  const span = ev.endD - start;
  if (!ev.data.rrule) return start < to && new Date(start.getTime() + span) > from ? [start] : [];
  const r = Object.fromEntries(String(ev.data.rrule).split(';').map((p) => p.split('=')));
  const interval = +r.INTERVAL || 1;
  const count = +r.COUNT || Infinity;
  const um = String(r.UNTIL || '').match(/^(\d{4})(\d{2})(\d{2})/);
  const until = um ? new Date(+um[1], +um[2] - 1, +um[3], 23, 59) : null;
  const out = [];
  let n = 0;
  const take = (d) => {
    if (until && d > until) return false;
    if (++n > count) return false;
    if (d >= from && d < to) out.push(new Date(d));
    return d < to;
  };
  if (r.FREQ === 'WEEKLY' && r.BYDAY) {
    const days = r.BYDAY.split(',').map((x) => WD[x.slice(-2)]).filter((x) => x !== undefined).sort();
    const week0 = addDays(start, -start.getDay());
    for (let w = 0; w < 3000; w += interval) {
      for (const wd of days) {
        const d = addDays(week0, w * 7 + wd);
        d.setHours(start.getHours(), start.getMinutes(), 0, 0);
        if (d < start) continue;
        if (!take(d)) return out;
      }
    }
    return out;
  }
  const d = new Date(start);
  for (let i = 0; i < 20000; i++) {
    if (!take(d)) break;
    if (r.FREQ === 'DAILY') d.setDate(d.getDate() + interval);
    else if (r.FREQ === 'WEEKLY') d.setDate(d.getDate() + 7 * interval);
    else if (r.FREQ === 'MONTHLY') d.setMonth(d.getMonth() + interval);
    else if (r.FREQ === 'YEARLY') d.setFullYear(d.getFullYear() + interval);
    else break;
  }
  return out;
}

function cmdAgenda(args) {
  const di = args.indexOf('--days');
  const days = di >= 0 ? Math.max(1, +args[di + 1] || 7) : 7;
  const past = args.includes('--past');   // also today's events that have already ended
  const now = new Date();
  const from = new Date(now); from.setHours(0, 0, 0, 0);
  const to = addDays(from, days);
  const rows = [];
  for (const ev of readEvents([])) {
    if (String(ev.data.status).toLowerCase() === 'cancelled') continue;
    const span = ev.endD - ev.start.d;
    for (const d of occurrences(ev, from, to)) {
      const end = new Date(d.getTime() + span);
      if (end <= now && !past) continue;
      const when = end <= now ? ' (ended)' : d <= now ? ' (now)' : '';
      const dayName = d.toLocaleDateString('en-GB', { weekday: 'short' });
      const time = (ev.allDay ? 'all day' : `${pad(d.getHours())}:${pad(d.getMinutes())}`) + when;
      const loc = ev.data.location ? ` (${ev.data.location})` : '';
      const rec = ev.data.rrule ? ' [recurring]' : '';
      rows.push([d.getTime(), `- ${dayName} ${isoDay(d)} ${time} ${ev.title}${loc}${rec} — ${rel(ev.file)}`]);
    }
  }
  rows.sort((a, b) => a[0] - b[0]);
  if (!rows.length) console.log(`(no events in the next ${days} days)`);
  for (const [, line] of rows) console.log(line);
}

// ---------- validate (offline; used by lint-check and check-file) ----------
function cmdValidate() {
  const problems = [];
  checkTodos(problems); readEvents(problems); checkContacts(problems);
  for (const p of problems) console.log(`- ${p}`);
  if (problems.length) process.exitCode = 1;
}

// ---------- main ----------
const [cmd, ...args] = process.argv.slice(2);
if (cmd === 'agenda') cmdAgenda(args);
else if (cmd === 'validate') cmdValidate();
else { console.error('usage: kioku-calendar.mjs agenda [--days N] [--past] | validate'); process.exit(2); }
