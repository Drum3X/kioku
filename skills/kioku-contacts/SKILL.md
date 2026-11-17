---
name: kioku-contacts
description: The user's contacts in their second brain (contacts/ in the Kioku vault), synced one-way to their CardDAV address book so they show in Thunderbird. Use when the user wants to add or update a person's contact details or look them up — "rehbere ekle", "Ahmet'in numarası şu", "Ayşe'nin maili değişti", "Ahmet'in maili neydi", "doğum günü" (or "add to my contacts", "what's X's email"). For people who are only a topic (an author, a public figure), use kioku-note instead.
argument-hint: "[person and details]"
---

# kioku-contacts

Vault: `$VAULT` — the vault path from the Kioku section of your global instructions (`~/.claude/CLAUDE.md` in Claude Code, `~/.gemini/GEMINI.md` in Antigravity).
Rules: `$VAULT/.kioku/SCHEMA.md` ("Calendar, todos and contacts"); personal rules in `$VAULT/CLAUDE.md`
CLI: `$VAULT/.kioku/bin/kioku`

Contacts live in the vault, one page per person: `contacts/<slug>.md`. After each commit a git
hook pushes them to the user's CardDAV address book, and Thunderbird shows them from there.
The address book's other contacts are never touched. An edit made in Thunderbird to a Kioku
contact is overwritten on the next sync, so changes always go through you.

## Add or update

1. Search first: `kioku search <name>`. If the person already has a page, update it instead of creating a second one.
2. New person: run `kioku new contact "<Full Name>"`.
3. Frontmatter:
   - `emails: [a@x.com, b@y.com]`
   - `phones: ["+90 555 123 45 67"]`, in international format when the country is clear
   - `org`
   - `birthday: YYYY-MM-DD`
   - `address`
   - `url`
   - `lang`: the conversation language
   - `tags`: for example `aile`, `üniversite`, `iş`
   Leave `uid` empty; the pre-commit hook fills it in.
4. Body:
   - `## How we know each other`: what the user said.
   - `## Notes`: anything else they want remembered.
   - Links to related projects or events.
5. Write only what the user gave you. Never guess an email address or a phone number.
6. Commit:
   ```
   git -C "$VAULT" add -A && git -C "$VAULT" commit -qm "contact: <name>"
   ```
7. Confirm in one line, in the user's language, for example: `Rehbere eklendi: Ahmet Yılmaz (telefon, e-posta)`.

## Look up

"What's Ahmet's email?": run `kioku search ahmet` and read the page. Contact pages are not in
the session-start context.

## Remove

Delete a contact only if the user explicitly asks. Delete the file and commit; the sync
removes the contact from the server.
