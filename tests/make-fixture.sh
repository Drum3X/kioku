#!/usr/bin/env bash
# Builds a test vault with tricky cases, dated around today, at $1 (linked to this checkout).
set -eu
V="$1"; SYS="$(cd "$(dirname "$0")/.." && pwd)"
d() { date -d "$1 days" +%F; }
T="$(d 0)"; Y="$(d -1)"; TM="$(d 1)"; T2="$(d 2)"; T5="$(d 5)"; LW="$(d -6)"
rm -rf "$V"; mkdir -p "$V"/{daily,journal,calendar/2026,contacts,projects,reviews,raw/inbox,wiki/{topics,concepts,entities,sources,ideas,synthesis}}
ln -s "$SYS" "$V/.kioku"
w() { mkdir -p "$(dirname "$V/$1")"; cat > "$V/$1"; }
w Core.md <<X
---
title: Core
---
# Core
## Identity
- Test user, prefers short answers.
<!-- a comment line -->
## Empty heading
X
w index.md <<X
# Index
## Topics
- [[wiki/topics/linux|Linux]]
- [[projects/alpha]]
X
w log.md <<X
# Log
## [$LW] ingest | Old thing
## [$Y] note | Yesterday thing
## [$T] lint | Today thing
X
w wiki/topics/linux.md <<X
---
title: Linux
type: topic
lang: en
created: $T
updated: $T
---
# Linux
- [[wiki/concepts/good]] — good page
- [[wiki/entities/İstanbul-tool|İstanbul]]
X
w wiki/concepts/good.md <<X
---
title: Good concept
type: concept
lang: en
tags: [kde, plasma]
created: $T
updated: $T
---
# Good concept
Topics: [[wiki/topics/linux|Linux]]
## How it works
Açılışta disk adları değişiyor; \`[[not-a-link]]\` in code is ignored.
See [[missing-page]] and [[wiki/concepts/good#How it works]] and [[good|alias]].
X
w wiki/entities/İstanbul-tool.md <<X
---
title: "İstanbul tool"
type: entity
lang: tr
created: $T
updated: $T
---
# İstanbul tool
Topics: [[wiki/topics/linux]]
ISPARTA ışık İĞDE
X
w wiki/entities/orphan.md <<X
---
title: Orphan
type: entity
created: $T
---
# Orphan
No topic line here.
X
w wiki/sources/src.md <<X
---
title: Source
type: source
lang: en
created: $T
updated: $T
---
# Source
Topics: [[wiki/topics/linux]]
X
w projects/alpha.md <<X
---
title: Alpha
type: project
status: active
path: ~/Projects/alpha
next: ship it
lang: en
created: $T
updated: $T
---
# Alpha
## Status
- Running.
X
w projects/secret.md <<X
---
title: Secret
type: project
status: active
private: true
lang: en
created: $T
updated: $T
---
# Secret
X
w daily/$Y.md <<X
---
title: $Y
---
# $Y
- 10:00 [alpha] learned: fixtures need edge cases ([[wiki/concepts/good]])
- 11:00 [alpha] learned: this one has no wiki link
X
w daily/$T.md <<X
# $T

- 09:00 [alpha] done: started
## Heading only

## Notes
- note
X
w journal/$T.md <<X
# $T
## Todos
- [ ] overdue thing (due: $Y) ^t-abc123
- [ ] soon thing (due: $TM) (at: 17:00) (remind: 2h)
- [ ] far thing (due: $T5)
- [ ] no due
- [ ] bad date (due: 2026-11-31)
- [ ] bad remind (due: $TM) (remind: bir saat)
- [ ] remind no due (remind: 1h)
- [x] done (due: $Y)
  - [ ] nested (due: $T2)
X
ev() { w "calendar/2026/$1.md"; }
ev today-meeting <<X
---
title: Meeting
type: event
start: ${T}T23:30
duration: 20m
location: Room 1
status: confirmed
---
X
ev weekly <<X
---
title: Class
type: event
start: $(d -14)T10:00
end: $(d -14)T11:30
rrule: FREQ=WEEKLY;BYDAY=MO,WE,FR
---
X
ev monthly <<X
---
title: Rent
type: event
start: 2026-01-31
all_day: true
rrule: FREQ=MONTHLY;COUNT=20
---
X
ev trip <<X
---
title: Trip
type: event
start: $TM
end: $T2
all_day: true
---
X
ev cancelled <<X
---
title: Cancelled
type: event
start: ${TM}T12:00
status: cancelled
---
X
ev daily-until <<X
---
title: Standup
type: event
start: $(d -3)T09:00
rrule: FREQ=DAILY;INTERVAL=2;UNTIL=$(date -d '4 days' +%Y%m%d)
---
X
ev bad-start <<X
---
title: Bad
type: event
start: 2026-02-30
---
X
ev bad-fields <<X
---
title: Bad fields
type: event
start: ${T2}T10:00
end: nope
duration: forever
rrule: WEEKLY
alarm: yarın
---
X
ev untitled <<X
---
type: event
start: ${T2}T08:00
---
X
w contacts/ahmet.md <<X
---
title: Ahmet
type: contact
birthday: 5 Mayıs
---
X
w raw/inbox/$T-090000-note.md <<X
---
type: capture
---
call the bank
X
touch "$V/raw/inbox/.gitkeep"
w reviews/2026/week.md <<X
---
title: Week review
type: review
created: $T
---
# Week review
Değerlendirme of the week.
X
