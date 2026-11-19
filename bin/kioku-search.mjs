#!/usr/bin/env node
// kioku search engine (no dependencies, Node 18+). Called through `kioku search`.
//
//   search [--raw] [-n N] <term...>
//
// Each argument is a term; an argument with a space or punctuation in it ("graphical-session",
// "dark mode") is matched as a phrase. A note does not need every term: notes are ranked by a
// BM25 score over title (x3), headings and tags (x2) and body, multiplied by the share of terms
// they match, a page-type weight (wiki/projects ahead of daily logs) and a small recency bonus.
// Matching ignores case and Turkish letters (İ/I/ı/i, ç/c, ş/s, ğ/g, ö/o, ü/u) and allows
// suffixes both ways ("açılışta" finds "açılış", "disk" finds "diskleri").

import fs from 'node:fs';
import path from 'node:path';

const VAULT = process.env.KIOKU_DIR || path.resolve(path.dirname(process.argv[1]), '../..');

// ---------- arguments ----------
const args = process.argv.slice(2);
let raw = false, limit = 20;
const terms = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--raw') raw = true;
  else if (a === '-n' || a === '--limit') limit = Math.max(1, parseInt(args[++i], 10) || 20);
  else terms.push(a);
}
if (!terms.length) { console.error('usage: kioku search [--raw] [-n N] <term...>'); process.exit(2); }

// ---------- text helpers ----------
const FOLD = { 'ç': 'c', 'ş': 's', 'ğ': 'g', 'ö': 'o', 'ü': 'u', 'â': 'a', 'î': 'i', 'û': 'u' };
const fold = (s) => s.replace(/[İIı]/g, 'i').toLowerCase().replace(/[çşğöüâîû]/g, (c) => FOLD[c]);
const WORD = /[\p{L}\p{N}]+/gu;
const words = (s) => fold(s).match(WORD) || [];

const STOP = new Set(words(`
  ve ile bir bu şu o da de mi mı mu mü ne neden niye nasıl hangi hangisi hangisiydi için gibi daha çok en
  ama veya ya ki her şey neydi nedir var yok ben sen biz siz benim bana beni bizim olan oldu olarak kadar
  diye mı acaba şimdi zaten artık hiç
  the a an and or of to in on at for with from by what how why which who did do does we i you my our is are
  was were be been it this that about can should would
`));

// ---------- query ----------
const query = [];
for (const t of terms) {
  const f = fold(t).trim();
  if (!f) continue;
  const ws = f.match(WORD) || [];
  if (ws.length === 1 && ws[0] === f) query.push({ text: t, word: f });
  else if (ws.length) query.push({ text: t, phrase: f });
}
// Drop stop words unless that would leave nothing.
let q = query.filter((x) => x.phrase || !STOP.has(x.word));
if (!q.length) q = query;
// Same word twice counts once.
q = q.filter((x, i) => q.findIndex((y) => (y.word || y.phrase) === (x.word || x.phrase)) === i);
if (!q.length) process.exit(0);

// How strongly doc token `d` matches query word `w`: 1 exact, 0.8 with a suffix either way, 0 none.
function wordMatch(w, d) {
  if (d === w) return 1;
  if (w.length >= 3 && d.startsWith(w)) return 0.8;
  if (d.length >= 4 && w.startsWith(d) && w.length - d.length <= 4) return 0.8;
  return 0;
}

// ---------- documents ----------
const DIRS = ['Core.md', 'projects', 'wiki', 'daily', 'journal', 'calendar', 'contacts', 'reviews', 'index.md'];
if (raw) DIRS.push('raw');
const files = [];
function walk(p) {
  let st; try { st = fs.statSync(p); } catch { return; }
  if (st.isDirectory()) {
    for (const e of fs.readdirSync(p)) if (!e.startsWith('.')) walk(path.join(p, e));
  } else if (p.endsWith('.md')) files.push(p);
}
for (const d of DIRS) walk(path.join(VAULT, d));

function parse(file) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split('\n');
  const fm = {};
  let i = 0;
  if (lines[0] === '---') {
    for (i = 1; i < lines.length && lines[i] !== '---'; i++) {
      const m = lines[i].match(/^([A-Za-z_]+):\s*(.*)$/);
      if (m) fm[m[1]] = m[2].replace(/^["']|["']$/g, '');
    }
    i++;
  }
  const body = lines.slice(i);
  const title = fm.title || '';
  const fields = { title: [title], head: [fm.tags || '', fm.aliases || ''], body: [] };
  for (const l of body) {
    if (/^# /.test(l)) fields.title.push(l.slice(2));
    else if (/^#{2,6} /.test(l)) fields.head.push(l.replace(/^#+ /, ''));
    else fields.body.push(l);
  }
  const counts = {};
  let len = 0;
  for (const [k, ls] of Object.entries(fields)) {
    const m = new Map();
    for (const w of words(ls.join('\n'))) { m.set(w, (m.get(w) || 0) + 1); len++; }
    counts[k] = m;
  }
  const folded = { title: fold(fields.title.join('\n')), head: fold(fields.head.join('\n')), body: fold(fields.body.join('\n')) };
  return { file, rel: path.relative(VAULT, file), fm, title, body, bodyStart: i, counts, folded, len };
}
const docs = files.map(parse);
if (!docs.length) process.exit(0);
const avgLen = docs.reduce((s, d) => s + d.len, 0) / docs.length;

const FIELD_W = { title: 3, head: 2, body: 1 };
function termFreq(doc, t) {
  let tf = 0;
  for (const [k, w] of Object.entries(FIELD_W)) {
    if (t.phrase) {
      let n = 0, at = -1;
      while ((at = doc.folded[k].indexOf(t.phrase, at + 1)) !== -1) n++;
      tf += w * n;
    } else {
      for (const [tok, c] of doc.counts[k]) {
        const m = wordMatch(t.word, tok);
        if (m) tf += w * m * c;
      }
    }
  }
  return tf;
}

function typeWeight(rel) {
  if (rel === 'Core.md' || rel.startsWith('projects/')) return 1.2;
  if (rel.startsWith('wiki/topics/')) return 1.0;
  if (rel.startsWith('wiki/')) return 1.2;
  if (rel.startsWith('daily/') || rel.startsWith('journal/')) return 0.85;
  if (rel === 'index.md') return 0.6;
  if (rel.startsWith('raw/')) return 0.7;
  return 1.0;
}
const today = Date.now();
function recency(doc) {
  const d = doc.fm.updated || doc.fm.date || doc.fm.created || (path.basename(doc.file).match(/^\d{4}-\d{2}-\d{2}/) || [])[0];
  const t = d ? Date.parse(d.slice(0, 10)) : NaN;
  if (Number.isNaN(t)) return 1;
  const days = (today - t) / 86400000;
  return days <= 14 ? 1.05 : days <= 90 ? 1.02 : 1;
}

// ---------- score ----------
const K1 = 1.2, B = 0.75, N = docs.length;
const tfs = docs.map((d) => q.map((t) => termFreq(d, t)));
const df = q.map((_, j) => tfs.filter((r) => r[j] > 0).length);
const idf = df.map((n) => Math.log(1 + (N - n + 0.5) / (n + 0.5)));

const results = [];
docs.forEach((d, i) => {
  let s = 0, matched = 0;
  q.forEach((_, j) => {
    const tf = tfs[i][j];
    if (!tf) return;
    matched++;
    s += idf[j] * (tf * (K1 + 1)) / (tf + K1 * (1 - B + B * d.len / avgLen));
  });
  if (!matched) return;
  s *= (matched / q.length) * typeWeight(d.rel) * recency(d);
  results.push({ d, s, matched });
});
results.sort((a, b) => b.s - a.s || a.d.rel.localeCompare(b.d.rel));

// ---------- print ----------
function lineHits(l) {
  const f = fold(l);
  return q.some((t) => t.phrase ? f.includes(t.phrase) : (f.match(WORD) || []).some((tok) => wordMatch(t.word, tok)));
}
for (const { d, s, matched } of results.slice(0, limit)) {
  const p = d.fm.private === 'true' ? ' [private]' : '';
  const m = q.length > 1 ? ` ${matched}/${q.length}` : '';
  console.log(`${d.rel} — ${d.title || '?'}${p}  (${s.toFixed(2)}${m})`);
  let shown = 0;
  d.body.forEach((l, n) => {
    if (shown >= 2 || !l.trim() || /^# /.test(l) || !lineHits(l)) return;
    console.log(`    ${n + 1 + d.bodyStart}:${l.trim().slice(0, 200)}`);
    shown++;
  });
}
