#!/usr/bin/env node
// kioku sync engine (no dependencies, Node 18+). Called through `kioku sync|agenda`.
//
//   sync [--check | --dry-run | --full | --quiet]   one-way push of journal todos, calendar
//                                                   events and contacts to CalDAV/CardDAV
//   ids [FILE...]                                   give todos/events/contacts stable ids
//   agenda [--days N] [--past]                      upcoming events (recurrences expanded; ended ones only with --past)
//
// Safety: only resources whose file name starts with "kioku-" (or the legacy "brain-") are ever written or deleted.

import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { execSync, execFileSync } from 'node:child_process';

const VAULT = process.env.KIOKU_DIR || path.resolve(path.dirname(process.argv[1]), '../..');
const CONF_FILE = path.join(process.env.XDG_CONFIG_HOME || path.join(os.homedir(), '.config'), 'kioku', 'sync.conf');
const STATE_DIR = path.join(process.env.XDG_STATE_HOME || path.join(os.homedir(), '.local', 'state'), 'kioku');
const STATE_FILE = path.join(STATE_DIR, 'sync-state.json');
const LOG_FILE = path.join(STATE_DIR, 'sync.log');
const LOCK_FILE = path.join(STATE_DIR, 'sync.lock');
const PREFIX = 'kioku-';
// Names used by earlier versions; still treated as ours so they can be cleaned up.
const LEGACY = ['brain-'];
const isMine = (name) => [PREFIX, ...LEGACY].some((p) => name.startsWith(p));

// ---------- small helpers ----------
const pad = (n) => String(n).padStart(2, '0');
const rel = (f) => path.relative(VAULT, f);
const sha = (s) => crypto.createHash('sha256').update(s).digest('hex');
const hex6 = () => crypto.randomBytes(3).toString('hex');
let quiet = false;
const say = (...a) => { if (!quiet) console.log(...a); };
function log(msg) {
  fs.mkdirSync(STATE_DIR, { recursive: true });
  fs.appendFileSync(LOG_FILE, `${new Date().toISOString()} ${msg}\n`);
}
function die(msg, code = 1) { console.error(`kioku sync: ${msg}`); log(`ERROR ${msg}`); process.exit(code); }

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
const list = (v) => (Array.isArray(v) ? v : v ? [v] : []).filter(Boolean);

// Markdown → plain text for descriptions.
function plain(md) {
  return md
    .replace(/\[\[([^\]|]+)\|([^\]]+)\]\]/g, '$2')
    .replace(/\[\[([^\]]+)\]\]/g, (_, p) => path.basename(p))
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, '$1 ($2)')
    .replace(/^#+\s*/gm, '')
    .replace(/^Topics:.*$/gm, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\n{3,}/g, '\n\n')
    .trim();
}

// Page body → description: drop the H1 and headings that have nothing under them.
function bodyText(md) {
  const lines = md.split(/\r?\n/).filter((l) => !/^# /.test(l));
  const out = [];
  for (let i = 0; i < lines.length; i++) {
    if (/^#{2,}\s/.test(lines[i])) {
      let j = i + 1;
      while (j < lines.length && !lines[j].trim()) j++;
      if (j >= lines.length || /^#{1,}\s/.test(lines[j])) continue;
    }
    out.push(lines[i]);
  }
  return plain(out.join("\n"));
}

// Local "YYYY-MM-DD" or "YYYY-MM-DD[T ]HH:MM" → { date, time, d }
function parseLocal(s) {
  const m = String(s || '').trim().match(/^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{1,2}):(\d{2}))?$/);
  if (!m) return null;
  const d = new Date(+m[1], +m[2] - 1, +m[3], +(m[4] || 0), +(m[5] || 0));
  if (isNaN(d)) return null;
  return { date: `${m[1]}-${m[2]}-${m[3]}`, time: m[4] ? `${pad(m[4])}:${m[5]}` : null, d };
}
const utc = (d) => `${d.getUTCFullYear()}${pad(d.getUTCMonth() + 1)}${pad(d.getUTCDate())}T${pad(d.getUTCHours())}${pad(d.getUTCMinutes())}${pad(d.getUTCSeconds())}Z`;
const ymd = (d) => `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}`;
const isoDay = (d) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
const addDays = (d, n) => { const x = new Date(d); x.setDate(x.getDate() + n); return x; };

// ---------- iCalendar / vCard ----------
const esc = (s) => String(s).replace(/\\/g, '\\\\').replace(/;/g, '\\;').replace(/,/g, '\\,').replace(/\r?\n/g, '\\n');
function fold(line) {
  if (Buffer.byteLength(line) <= 75) return line;
  const parts = [];
  let cur = '', len = 0;
  for (const ch of line) {
    const l = Buffer.byteLength(ch);
    if (len + l > (parts.length ? 74 : 75)) { parts.push(cur); cur = ''; len = 0; }
    cur += ch; len += l;
  }
  parts.push(cur);
  return parts.join('\r\n ');
}
const serialize = (lines) => lines.filter((l) => l !== null && l !== undefined).map(fold).join('\r\n') + '\r\n';
const vcal = (inner) => ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//kioku//kioku-sync//EN', ...inner, 'END:VCALENDAR'];

// ---------- read the vault ----------
const TODO_RE = /^(\s*- \[)([ xX-])(\] )(.*)$/;
const TODO_ID_RE = /\s\^(t-[0-9a-z]+)\s*$/;

function readTodos(problems) {
  const items = [];
  for (const f of mdFiles(path.join(VAULT, 'journal'))) {
    const fileDay = path.basename(f, '.md');
    fs.readFileSync(f, 'utf8').split('\n').forEach((line, i) => {
      const m = line.match(TODO_RE);
      if (!m) return;
      let text = m[4];
      const id = (text.match(TODO_ID_RE) || [])[1];
      if (!id) { problems.push(`${rel(f)}:${i + 1}: todo without an id (run: kioku sync --ids)`); return; }
      text = text.replace(TODO_ID_RE, '');
      const due = (text.match(/\(due:\s*(\d{4}-\d{2}-\d{2})\)/) || [])[1];
      const at = (text.match(/\(at:\s*(\d{1,2}:\d{2})\)/) || [])[1];
      const done = (text.match(/\(done:\s*(\d{4}-\d{2}-\d{2})\)/) || [])[1];
      const remindRaw = (text.match(/\(remind:\s*([^)]*)\)/) || [])[1];
      const summary = plain(text.replace(/\((due|at|done|cancelled|remind):[^)]*\)/g, '')).replace(/\s+/g, ' ').trim();
      const status = m[2] === ' ' ? 'NEEDS-ACTION' : m[2] === '-' ? 'CANCELLED' : 'COMPLETED';
      const stamp = parseLocal(fileDay) ? utc(parseLocal(fileDay).d) : '19700101T000000Z';
      const lines = ['BEGIN:VTODO', `UID:${PREFIX}${id}`, `DTSTAMP:${stamp}`, `CREATED:${stamp}`, `SUMMARY:${esc(summary)}`];
      if (due) {
        const dd = parseLocal(at ? `${due}T${at}` : due);
        if (dd) lines.push(dd.time ? `DUE:${utc(dd.d)}` : `DUE;VALUE=DATE:${ymd(dd.d)}`);
        else problems.push(`${rel(f)}:${i + 1}: invalid due date`);
      }
      lines.push(`STATUS:${status}`);
      if (status === 'COMPLETED') {
        lines.push('PERCENT-COMPLETE:100');
        const dd = parseLocal(done || fileDay);
        if (dd) lines.push(`COMPLETED:${utc(new Date(dd.d.getTime() + 12 * 3600e3))}`);
      }
      // Reminder: open todos with a due time ring at that time; "(remind: 1h)" rings earlier.
      // A date-only due date rings only with remind:, counted from 09:00 that day.
      if (status === 'NEEDS-ACTION' && due) {
        let offset = null;
        if (remindRaw !== undefined) {
          const rm = remindRaw.trim().match(/^(\d+)\s*([mhd])$|^(0|now)$/);
          if (rm) offset = rm[3] ? 0 : +rm[1] * { m: 60e3, h: 3600e3, d: 86400e3 }[rm[2]];
          else problems.push(`${rel(f)}:${i + 1}: remind must look like 30m, 2h, 1d or 0`);
        } else if (at) offset = 0;
        const base = parseLocal(`${due}T${at || '09:00'}`);
        if (offset !== null && base) {
          lines.push('BEGIN:VALARM', 'ACTION:DISPLAY', `DESCRIPTION:${esc(summary)}`,
            `TRIGGER;VALUE=DATE-TIME:${utc(new Date(base.d.getTime() - offset))}`, 'END:VALARM');
        }
      } else if (remindRaw !== undefined && !due) {
        problems.push(`${rel(f)}:${i + 1}: remind needs a due date`);
      }
      lines.push(`DESCRIPTION:${esc(`Kioku: ${rel(f)}`)}`, 'END:VTODO');
      items.push({ kind: 'calendar', uid: `${PREFIX}${id}`, src: `${rel(f)}:${i + 1}`, body: serialize(vcal(lines)) });
    });
  }
  return items;
}

function readEvents(problems) {
  const events = [];
  for (const f of mdFiles(path.join(VAULT, 'calendar'))) {
    const { data, body } = parseFrontmatter(fs.readFileSync(f, 'utf8'));
    if (data.type && data.type !== 'event') continue;
    const start = parseLocal(data.start);
    if (!start) { problems.push(`${rel(f)}: invalid or missing start (YYYY-MM-DD or YYYY-MM-DDTHH:MM)`); continue; }
    const allDay = String(data.all_day) === 'true' || !start.time;
    let end = data.end ? parseLocal(data.end) : null;
    if (data.end && !end) problems.push(`${rel(f)}: invalid end`);
    let endD;
    if (allDay) endD = addDays(end ? end.d : start.d, 1);               // DTEND is exclusive
    else if (end && end.time) endD = end.d;
    else {
      const dm = String(data.duration || '').match(/^(\d+)\s*(m|h)$/);
      endD = new Date(start.d.getTime() + (dm ? +dm[1] * (dm[2] === 'h' ? 3600e3 : 60e3) : 3600e3));
    }
    events.push({ file: f, data, body, start, allDay, endD, title: data.title || path.basename(f, '.md') });
  }
  return events;
}

function eventItems(events, problems) {
  const items = [];
  for (const ev of events) {
    const { data } = ev;
    if (String(data.status).toLowerCase() === 'cancelled') continue;   // removed from the server
    if (!data.uid) { problems.push(`${rel(ev.file)}: event without uid (run: kioku sync --ids)`); continue; }
    const stampSrc = parseLocal(data.updated || data.created || ev.start.date);
    const lines = ['BEGIN:VEVENT', `UID:${PREFIX}${data.uid}`, `DTSTAMP:${utc((stampSrc || ev.start).d)}`, `SUMMARY:${esc(ev.title)}`];
    if (ev.allDay) lines.push(`DTSTART;VALUE=DATE:${ymd(ev.start.d)}`, `DTEND;VALUE=DATE:${ymd(ev.endD)}`);
    else lines.push(`DTSTART:${utc(ev.start.d)}`, `DTEND:${utc(ev.endD)}`);
    if (data.rrule) {
      if (/^FREQ=[A-Z]+/.test(data.rrule)) lines.push(`RRULE:${data.rrule}`);
      else problems.push(`${rel(ev.file)}: rrule must look like FREQ=WEEKLY;BYDAY=MO`);
    }
    if (data.location) lines.push(`LOCATION:${esc(data.location)}`);
    if (String(data.status).toLowerCase() === 'tentative') lines.push('STATUS:TENTATIVE');
    const desc = bodyText(ev.body);
    lines.push(`DESCRIPTION:${esc((desc ? desc + '\n\n' : '') + `Kioku: ${rel(ev.file)}`)}`);
    const am = String(data.alarm || '').match(/^(\d+)\s*([mhd])$/);
    if (am) {
      const trig = am[2] === 'd' ? `-P${am[1]}D` : `-PT${am[1]}${am[2] === 'h' ? 'H' : 'M'}`;
      lines.push('BEGIN:VALARM', 'ACTION:DISPLAY', `DESCRIPTION:${esc(ev.title)}`, `TRIGGER:${trig}`, 'END:VALARM');
    } else if (data.alarm) problems.push(`${rel(ev.file)}: alarm must look like 30m, 2h or 1d`);
    lines.push('END:VEVENT');
    items.push({ kind: 'calendar', uid: `${PREFIX}${data.uid}`, src: rel(ev.file), body: serialize(vcal(lines)) });
  }
  return items;
}

function readContacts(problems) {
  const items = [];
  for (const f of mdFiles(path.join(VAULT, 'contacts'))) {
    const { data, body } = parseFrontmatter(fs.readFileSync(f, 'utf8'));
    if (data.type && data.type !== 'contact') continue;
    if (!data.uid) { problems.push(`${rel(f)}: contact without uid (run: kioku sync --ids)`); continue; }
    const fn = data.title || path.basename(f, '.md');
    const parts = fn.trim().split(/\s+/);
    const family = parts.length > 1 ? parts.pop() : '';
    const lines = ['BEGIN:VCARD', 'VERSION:3.0', 'PRODID:-//kioku//kioku-sync//EN', `UID:${PREFIX}${data.uid}`, `FN:${esc(fn)}`, `N:${esc(family)};${esc(parts.join(' '))};;;`];
    if (data.org) lines.push(`ORG:${esc(data.org)}`);
    for (const e of list(data.emails || data.email)) lines.push(`EMAIL;TYPE=INTERNET:${esc(e)}`);
    for (const p of list(data.phones || data.phone)) lines.push(`TEL;TYPE=CELL:${esc(p)}`);
    if (data.birthday) {
      if (parseLocal(data.birthday)) lines.push(`BDAY:${data.birthday}`);
      else problems.push(`${rel(f)}: birthday must be YYYY-MM-DD`);
    }
    if (data.address) lines.push(`ADR;TYPE=HOME:;;${esc(data.address)};;;;`);
    if (data.url) lines.push(`URL:${data.url}`);
    const note = bodyText(body);
    if (note) lines.push(`NOTE:${esc(note.slice(0, 4000))}`);
    lines.push('END:VCARD');
    items.push({ kind: 'addressbook', uid: `${PREFIX}${data.uid}`, src: rel(f), body: serialize(lines) });
  }
  return items;
}

// ---------- ids ----------
function cmdIds(files) {
  const targets = files.length
    ? files.map((p) => path.resolve(VAULT, p)).filter((p) => fs.existsSync(p) && p.endsWith('.md'))
    : [...mdFiles(path.join(VAULT, 'journal')), ...mdFiles(path.join(VAULT, 'calendar')), ...mdFiles(path.join(VAULT, 'contacts'))];
  const used = new Set();
  for (const f of [...mdFiles(path.join(VAULT, 'journal')), ...mdFiles(path.join(VAULT, 'calendar')), ...mdFiles(path.join(VAULT, 'contacts'))]) {
    for (const m of fs.readFileSync(f, 'utf8').matchAll(/\^(t-[0-9a-z]+)|^uid:[ \t]*([ec]-[0-9a-z]+)/gm)) used.add(m[1] || m[2]);
  }
  const fresh = (p) => { let id; do id = `${p}-${hex6()}`; while (used.has(id)); used.add(id); return id; };
  const changed = [];
  for (const f of targets) {
    const r = rel(f);
    const text = fs.readFileSync(f, 'utf8');
    let out = text;
    if (r.startsWith('journal/')) {
      out = text.split('\n').map((line) => {
        const m = line.match(TODO_RE);
        return m && !TODO_ID_RE.test(m[4]) ? `${line.replace(/\s+$/, '')} ^${fresh('t')}` : line;
      }).join('\n');
    } else if (r.startsWith('calendar/') || r.startsWith('contacts/')) {
      const fm = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
      if (fm && !/^uid:[ \t]*\S+/m.test(fm[1])) {
        const id = fresh(r.startsWith('calendar/') ? 'e' : 'c');
        const head = /^uid:[ \t]*$/m.test(fm[1]) ? fm[1].replace(/^uid:[ \t]*$/m, `uid: ${id}`) : `${fm[1]}\nuid: ${id}`;
        out = text.replace(fm[0], `---\n${head}\n---`);
      }
    }
    if (out !== text) { fs.writeFileSync(f, out); changed.push(r); }
  }
  for (const c of changed) console.log(c);
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

// ---------- DAV ----------
function readConf() {
  if (!fs.existsSync(CONF_FILE)) die(`no config at ${CONF_FILE} (see the README: Calendar, contacts and todos)`, 3);
  const conf = {};
  for (const line of fs.readFileSync(CONF_FILE, 'utf8').split('\n')) {
    const m = line.match(/^\s*([a-z_]+)\s*=\s*(.*?)\s*$/);
    if (m && !line.trim().startsWith('#')) conf[m[1]] = m[2];
  }
  if (!conf.url || !conf.user) die(`${CONF_FILE} needs at least url= and user=`);
  conf.calendar ||= 'personal';
  conf.addressbook ||= 'personal';
  return conf;
}
// Password lookup, in order: KIOKU_DAV_PASSWORD (tests, kioku password set), password_cmd (any
// command that prints it), password_store=keyring (Secret Service on Linux, Keychain on macOS;
// written by `kioku password set`), password_file (default ~/.config/kioku/dav.pass). The file
// must not be readable by others (chmod 600); this is checked on POSIX systems.
const DEFAULT_PASS_FILE = path.join(path.dirname(CONF_FILE), 'dav.pass');
const KEYRING_SERVICE = 'kioku';
function keyringLookup(user) {
  const [cmd, args] = process.platform === 'darwin'
    ? ['security', ['find-generic-password', '-s', KEYRING_SERVICE, '-a', user, '-w']]
    : ['secret-tool', ['lookup', 'service', KEYRING_SERVICE, 'account', user]];
  try { return execFileSync(cmd, args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).replace(/\r?\n$/, ''); }
  catch { die(`no password in the keyring for ${user} (locked, unavailable or not set); run: kioku password status`, 3); }
}
function password(conf) {
  if (process.env.KIOKU_DAV_PASSWORD !== undefined) return process.env.KIOKU_DAV_PASSWORD;
  if (conf.password_cmd) {
    try { return execSync(conf.password_cmd, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).replace(/\r?\n$/, ''); }
    catch { die('password_cmd failed (run it yourself to see why)'); }
  }
  if (conf.password_store === 'keyring') return keyringLookup(conf.user);
  const file = (conf.password_file || DEFAULT_PASS_FILE).replace(/^~(?=$|\/)/, os.homedir());
  if (!fs.existsSync(file)) {
    if (conf.password_file) die(`password_file not found: ${file}`);
    die(`no password; run: kioku password set`, 3);
  }
  if (process.platform !== 'win32' && (fs.statSync(file).mode & 0o077)) {
    die(`${file} is readable by other users; run: chmod 600 ${file}`);
  }
  try { return fs.readFileSync(file, 'utf8').replace(/\r?\n$/, ''); }
  catch { die(`cannot read ${file}`); }
}

function makeClient(conf) {
  const pw = password(conf);
  const auth = pw === null ? null : 'Basic ' + Buffer.from(`${conf.user}:${pw}`).toString('base64');
  return async (method, url, { body, headers = {}, depth } = {}) => {
    const h = { ...headers };
    if (auth) h.Authorization = auth;
    if (depth !== undefined) h.Depth = String(depth);
    let res;
    try { res = await fetch(url, { method, headers: h, body, redirect: 'follow' }); }
    catch (e) { throw new Error(`${method} ${url}: ${e.cause?.code || e.message}`); }
    const text = await res.text();
    if (res.status === 401) throw new Error(`${method} ${url}: 401 unauthorized (check user and app password)`);
    return { status: res.status, text };
  };
}

const T = (tag) => `<(?:[A-Za-z0-9_-]+:)?${tag}(?:\\s[^>]*)?`;
const responses = (xml) => xml.split(new RegExp(T('response') + '>', 'i')).slice(1);
const firstHref = (s) => ((s || '').match(new RegExp(T('href') + '>\\s*([^<\\s]+)\\s*<', 'i')) || [])[1];
const propBlock = (s, tag) => ((s || '').match(new RegExp(T(tag) + '>([\\s\\S]*?)</(?:[A-Za-z0-9_-]+:)?' + tag + '>', 'i')) || [])[1];
const hasTag = (s, tag) => new RegExp(T(tag) + '\\s*/?>', 'i').test(s || '');

async function discover(dav, conf, kind) {
  if (conf[`${kind}_url`]) return conf[`${kind}_url`].replace(/\/?$/, '/');
  const base = conf.url.replace(/\/?$/, '/');
  const pf = (props) => `<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:a="urn:ietf:params:xml:ns:carddav"><d:prop>${props}</d:prop></d:propfind>`;
  const hdr = { 'Content-Type': 'application/xml; charset=utf-8' };
  const r1 = await dav('PROPFIND', base, { depth: 0, headers: hdr, body: pf('<d:current-user-principal/>') });
  const principal = firstHref(propBlock(r1.text, 'current-user-principal'));
  if (!principal) throw new Error(`no current-user-principal at ${base} (HTTP ${r1.status})`);
  const purl = new URL(principal, base).href;
  const homeTag = kind === 'calendar' ? 'calendar-home-set' : 'addressbook-home-set';
  const r2 = await dav('PROPFIND', purl, { depth: 0, headers: hdr, body: pf(kind === 'calendar' ? '<c:calendar-home-set/>' : '<a:addressbook-home-set/>') });
  const home = firstHref(propBlock(r2.text, homeTag));
  if (!home) throw new Error(`no ${homeTag} for ${purl}`);
  const hurl = new URL(home, purl).href;
  const r3 = await dav('PROPFIND', hurl, { depth: 1, headers: hdr, body: pf('<d:displayname/><d:resourcetype/>') });
  const found = [];
  for (const resp of responses(r3.text)) {
    const href = firstHref(resp);
    const rt = propBlock(resp, 'resourcetype') || '';
    if (!href || !hasTag(rt, kind)) continue;
    const name = (propBlock(resp, 'displayname') || '').trim();
    const seg = decodeURIComponent(href.replace(/\/$/, '').split('/').pop());
    found.push({ url: new URL(href, hurl).href.replace(/\/?$/, '/'), name, seg });
  }
  const want = conf[kind];
  const hit = found.find((c) => c.seg === want) || found.find((c) => c.name === want);
  if (!hit) throw new Error(`no ${kind} named "${want}" (found: ${found.map((c) => `${c.seg}${c.name ? ` "${c.name}"` : ''}`).join(', ') || 'none'})`);
  return hit.url;
}

async function listCollection(dav, url) {
  const body = '<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:getetag/></d:prop></d:propfind>';
  const r = await dav('PROPFIND', url, { depth: 1, headers: { 'Content-Type': 'application/xml; charset=utf-8' }, body });
  if (r.status >= 300) throw new Error(`PROPFIND ${url}: HTTP ${r.status}`);
  const all = [];
  for (const resp of responses(r.text)) {
    const href = firstHref(resp);
    if (!href) continue;
    const abs = new URL(href, url).href;
    if (abs.replace(/\/?$/, '/') === url) continue;
    all.push(abs);
  }
  const mine = all.filter((h) => isMine(decodeURIComponent(h.split('/').pop())));
  return { all, mine };
}

function assertMine(href) {
  if (!isMine(decodeURIComponent(new URL(href).pathname.split('/').pop()))) throw new Error(`refusing to touch ${href}: not a ${PREFIX} resource`);
}

let locked = false;
function lock() {
  fs.mkdirSync(STATE_DIR, { recursive: true });
  const t0 = Date.now();
  for (;;) {
    try { fs.writeFileSync(LOCK_FILE, String(process.pid), { flag: 'wx' }); locked = true; return; }
    catch {
      let stale = true;
      try { stale = Date.now() - fs.statSync(LOCK_FILE).mtimeMs > 120e3; } catch {}
      if (stale) { try { fs.unlinkSync(LOCK_FILE); } catch {} continue; }
      if (Date.now() - t0 > 90e3) die('another sync is still running');
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 1000);
    }
  }
}
const unlock = () => { if (!locked) return; locked = false; try { fs.unlinkSync(LOCK_FILE); } catch {} };

async function cmdSync(args) {
  quiet = args.includes('--quiet');
  const check = args.includes('--check'), dry = args.includes('--dry-run'), full = args.includes('--full');
  const problems = [];
  const items = [...readTodos(problems), ...eventItems(readEvents(problems), problems), ...readContacts(problems)];
  const conf = readConf();
  const dav = makeClient(conf);
  const urls = { calendar: await discover(dav, conf, 'calendar'), addressbook: await discover(dav, conf, 'addressbook') };
  const ext = { calendar: '.ics', addressbook: '.vcf' };
  const ctype = { calendar: 'text/calendar; charset=utf-8', addressbook: 'text/vcard; charset=utf-8' };
  for (const it of items) { it.href = urls[it.kind] + encodeURIComponent(it.uid) + ext[it.kind]; it.hash = sha(it.body); }

  if (check) {
    const cal = await listCollection(dav, urls.calendar), ab = await listCollection(dav, urls.addressbook);
    console.log(`calendar:     ${urls.calendar}\n              ${cal.all.length} items, ${cal.mine.length} from Kioku`);
    console.log(`address book: ${urls.addressbook}\n              ${ab.all.length} items, ${ab.mine.length} from Kioku`);
    const n = (k, p) => items.filter((i) => i.kind === k && i.uid.startsWith(p)).length;
    console.log(`vault:        ${n('calendar', PREFIX + 't-')} todos, ${n('calendar', PREFIX + 'e-')} events, ${n('addressbook', PREFIX)} contacts`);
    for (const p of problems) console.log(`warning: ${p}`);
    return;
  }

  lock();
  try {
    let state = {};
    try { state = JSON.parse(fs.readFileSync(STATE_FILE, 'utf8')); } catch {}
    if (state.vault !== VAULT || state.calendar !== urls.calendar || state.addressbook !== urls.addressbook) state = { vault: VAULT, ...urls, items: {} };
    state.items ||= {};
    const want = new Map(items.map((i) => [i.href, i]));
    const del = new Set(Object.keys(state.items).filter((h) => !want.has(h)));
    // --full, or the first sync after the resource prefix changed: remove our stray items.
    if (full || state.prefix !== PREFIX) {
      for (const u of Object.values(urls)) for (const h of (await listCollection(dav, u)).mine) if (!want.has(h)) del.add(h);
    }
    state.prefix = PREFIX;
    const put = items.filter((i) => full || state.items[i.href]?.hash !== i.hash);
    let ok = 0, failed = 0;
    for (const it of put) {
      assertMine(it.href);
      if (dry) { say(`PUT    ${it.href.split('/').pop()}  (${it.src})`); continue; }
      const r = await dav('PUT', it.href, { headers: { 'Content-Type': ctype[it.kind] }, body: it.body });
      if (r.status >= 200 && r.status < 300) { state.items[it.href] = { hash: it.hash, src: it.src }; ok++; }
      else { failed++; say(`failed PUT ${it.src}: HTTP ${r.status} ${r.text.slice(0, 200).replace(/\s+/g, ' ')}`); log(`PUT ${it.href} HTTP ${r.status}`); }
    }
    for (const h of del) {
      assertMine(h);
      if (dry) { say(`DELETE ${h.split('/').pop()}`); continue; }
      const r = await dav('DELETE', h);
      if ((r.status >= 200 && r.status < 300) || r.status === 404) { delete state.items[h]; ok++; }
      else { failed++; say(`failed DELETE ${h}: HTTP ${r.status}`); log(`DELETE ${h} HTTP ${r.status}`); }
    }
    if (!dry) {
      fs.mkdirSync(STATE_DIR, { recursive: true });
      fs.writeFileSync(STATE_FILE, JSON.stringify(state, null, 1));
    }
    for (const p of problems) say(`warning: ${p}`);
    const summary = `${dry ? 'dry run: ' : ''}${put.length} to send, ${del.size} to delete${dry ? '' : `; ${ok} done, ${failed} failed`}`;
    say(summary);
    log(`sync ${summary}${problems.length ? `; ${problems.length} warning(s)` : ''}`);
    if (failed) process.exitCode = 1;
  } finally { unlock(); }
}

// ---------- validate (offline; used by lint-check) ----------
function cmdValidate() {
  const problems = [];
  readTodos(problems); eventItems(readEvents(problems), problems); readContacts(problems);
  for (const p of problems) console.log(`- ${p}`);
  if (problems.length) process.exitCode = 1;
}

// ---------- main ----------
const [cmd, ...args] = process.argv.slice(2);
try {
  if (cmd === 'sync') await cmdSync(args);
  else if (cmd === 'ids') cmdIds(args);
  else if (cmd === 'agenda') cmdAgenda(args);
  else if (cmd === 'validate') cmdValidate();
  else { console.error('usage: kioku-sync.mjs sync [--check|--dry-run|--full|--quiet] | ids [FILE...] | agenda [--days N] [--past] | validate'); process.exit(2); }
} catch (e) {
  unlock();
  die(e.message);
}
