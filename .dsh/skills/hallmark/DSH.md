# Hallmark on DeepSeek Harness (DSH)

This directory is a vendored copy of [Nutlope/hallmark](https://github.com/Nutlope/hallmark)
adapted for DeepSeek Harness. Upstream ships installers only for Claude Code, Cursor, and
Codex (`npx skills add nutlope/hallmark`); DSH has its own discovery rules and tool names,
so the bundle is installed and patched by hand.

## Why a manual install

DSH's filesystem skill provider scans, in order:

| Rank | Root |
| --- | --- |
| 100 | `<projectRoot>/.dsh/skills` |
| 200 | `<projectRoot>/.agents/skills` |
| 300 | configured `customSkillDirs` |
| 400 | `~/.dsh/skills` |
| 500 | `~/.agents/skills` |

Only one level is scanned: `<root>/<name>/SKILL.md` (a directory bundle) or
`<root>/<name>.md` (a flat file). Nested `**/SKILL.md` is ignored. This copy sits at
`.dsh/skills/hallmark/SKILL.md`, so it is project-scoped: it is available in AlphaK, not
in every DSH project. Move/copy it to `~/.dsh/skills/hallmark/` to make it user-wide.

`projectRoot` is the nearest ancestor containing `.git`; if none exists, DSH uses the
session's current working directory. AlphaK has no `.git` today, so a session started in
a subdirectory would not see this skill. Start sessions at the project root, or run
`git init` to pin the root.

## Tool mapping

The bundle is written in Claude Code vocabulary. On DSH:

| Upstream wording | DSH tool | Notes |
| --- | --- | --- |
| `Read` | `read` | path + optional offset/limit |
| `Write` / `Edit` | `write` / `edit` | `write` needs the full file; `edit` is literal replace |
| `Bash` | `bash` | runs in a fresh shell; pass `workdir` instead of `cd` |
| `WebFetch` | `web_fetch` | one URL, returns decoded text only |
| `Grep` / glob | `grep` / `glob` | use these instead of shell `grep`/`find` |
| screenshot / attached image | `read_image` | user-supplied images only |
| subagent / Task | `subagent` | Hallmark does not need it; single-threaded is fine |
| deliverable hand-off | `present` | optional file card for emitted HTML/CSS |

`WebFetch` has already been rewritten to `web_fetch` in `SKILL.md` and
`references/study.md`; everything else keeps upstream wording.

## Output contract on DSH

- **Write real files.** A Hallmark run is a coding task: emit the page, component,
  `tokens.css`, and `.hallmark/log.json` with `write`/`edit`. Paths resolve from the
  session workspace (AlphaK root).
- **Do not emit a `schemaJson` card for a Hallmark build.** DSH's generative-UI card
  format is for chat answers, not for project code. If a short chat summary helps, give
  it as prose after the files — never wrap the design in a card.
- **Card styling rules do not apply.** Hallmark's CSS conventions (palette, background,
  fonts) govern project files; DSH's restrictions on `schemaJson` components do not.
- **No fabricated proof.** Hallmark already bans invented metrics, logos, and
  testimonials. Keep that, and never claim a page was rendered, screenshotted, or
  visually verified when it was not.
- **No dialogs.** DSH forbids `alert`/`confirm`/`prompt` in generated logic. Hallmark
  already prefers optimistic update + Undo over confirmation dialogs.
- **Write scope.** The workspace is writable; the skill bundle and DSH runtime are
  read-only resources. Keep generated work inside the project.

## Known gaps in this host

1. **`study` URL mode is weaker.** Upstream tells the model to ask `WebFetch` for the
   page HTML *plus* same-origin stylesheets in one call. DSH's `web_fetch` returns a
   single URL's decoded text — no headers, no status, no bundled CSS. Practical flow:
   fetch the page, then fetch the `<link rel="stylesheet">` URLs one by one (cap at the
   handful that carry tokens/typography), or ask the user for a screenshot. If the body
   is an auth wall, an SPA shell, or under ~1 KB, fall back to the screenshot path per
   `references/study.md`. Do not pretend rhythm was observed from HTML.
2. **No browser, screenshot, or image generation.** `read_image` only reads an existing
   image file. Hallmark's "verify at 320 / 375 / 414 / 768 px" is a source-level
   discipline here: reason about the CSS, state that visual verification is pending, and
   ask the user to check in a browser.
3. **No rendered visual check.** This host has no page renderer or headless browser.
   Do not claim a screenshot, a viewport check, or a pixel diff.
4. **Skill body is large.** Loading `hallmark` returns the full ~67 KB `SKILL.md`. The
   eager/lazy loading table in the skill is authoritative — load only the reference
   files a run actually needs.
5. **Project memory needs a stable root.** `.hallmark/log.json` and `design.md` live at
   the project root. Without `.git`, that root is whatever directory the session started
   in; keep it consistent so diversification memory works.

## Verify the install

From the project root:

```bash
test -f .dsh/skills/hallmark/SKILL.md && echo ok
sed -n '1,5p' .dsh/skills/hallmark/SKILL.md   # name + description frontmatter
grep -c "web_fetch" .dsh/skills/hallmark/references/study.md
```

In a DSH session, the catalog lists `hallmark`; load it with `skill hallmark` or invoke
`/hallmark`. A successful load returns the full instruction body.

## Updating

1. Fetch upstream `skills/hallmark/` from `Nutlope/hallmark@main` (GitHub may need a
   mirror in this network; the upstream tarball is
   `https://github.com/Nutlope/hallmark/archive/refs/heads/main.tar.gz`).
2. Replace `SKILL.md` and `references/` with the new upstream copy.
3. Re-apply this adapter:
   - replace `WebFetch` with `web_fetch` in `SKILL.md` and `references/study.md`;
   - re-insert the "Host adapter — DeepSeek Harness (DSH)" block before
     `## How to use this skill` in `SKILL.md`;
   - keep this `DSH.md` and update the provenance below.
4. Re-run the verification commands.

## Provenance

- Upstream: `https://github.com/Nutlope/hallmark` (`main`, vendored 2026-10-21)
- Skill version in frontmatter: `1.1.0`
- License: MIT (see `LICENSE`)
- Local edits: host-adapter block in `SKILL.md`; `WebFetch` → `web_fetch` in `SKILL.md`
  and `references/study.md`; this file.
