# OpenSpec on DeepSeek Harness (DSH)

Status: **working** via a project wrapper. The six `openspec-*` skills were
discovered by DSH already; the reason they "did nothing" was the CLI, not the
skills.

## Symptom and root cause

Any `openspec` command in DSH died before printing help:

```
SyntaxError: Invalid regular expression flags
  .../string-width/index.js:20
const visibleCharacterRegex = /[^\p{...}]/v;
```

- DSH's `bash` inherits a PATH whose `node` is **v18.20.5** (nvm), and the
  global `openspec` launcher is `#!/usr/bin/env node`.
- OpenSpec **1.14.0** declares `engines.node >= 20.19.0`. Its `string-width`
  dependency uses the RegExp `v` flag, which Node 18 cannot parse.
- So the CLI crashed on load. DSH still listed the skills because skill
  discovery only parses YAML frontmatter — it never runs the CLI.

Node 20.20.2 is already installed at
`~/.nvm/versions/node/v20.20.2/bin/node`, but it is not the `node` that DSH's
non-interactive shell resolves.

## The fix: `.dsh/bin/openspec`

`.dsh/bin/openspec` is a small launcher that:

1. finds a Node with major version >= 20 (PATH first, then nvm v24/v22/v20);
2. finds the OpenSpec CLI (project `node_modules`, any nvm global install,
   `/usr/local`, Homebrew);
3. `exec`s the CLI with that Node.

Use it for **every** OpenSpec command in this project:

```bash
.dsh/bin/openspec list --json
.dsh/bin/openspec context --json
.dsh/bin/openspec status --change "<name>" --json
.dsh/bin/openspec new change "<name>"
```

Do **not** run bare `openspec` in DSH. From a different working directory use
the absolute path:

```bash
/Users/ryan/Desktop/Project/AlphaK/.dsh/bin/openspec list --json
```

Overrides:

| Variable | Purpose |
| --- | --- |
| `OPENSPEC_NODE` | explicit Node binary (must be >= 20.19) |
| `OPENSPEC_CLI` | explicit path to `bin/openspec.js` |

## DSH ↔ OpenSpec host mapping

| Upstream (Claude Code / Cursor) | DSH |
| --- | --- |
| `/opsx:propose`, `/opsx propose` | `/openspec-propose` (skill name), or plain language |
| `/opsx:apply` | `/openspec-apply-change` |
| `/opsx:explore` | `/openspec-explore` |
| `/opsx:update` | `/openspec-update-change` |
| `/opsx:sync` | `/openspec-sync-specs` |
| `/opsx:archive` | `/openspec-archive-change` |
| `allowed-tools: Bash(openspec:*)` | ignored by DSH; every skill can use `bash` |
| `Read` / `Write` / `Edit` | `read` / `write` / `edit` |
| `AskUserQuestion` | ask in normal assistant text |
| `TodoWrite` | `todo_write` (optional) |

DSH has no colon-style commands: typing `/opsx propose` is treated as plain
text, not a skill invocation. Natural-language "opsx propose" can still select
the skill because its catalog description mentions it, but the reliable entry
points are `/openspec-propose` and loading the skill by name.

Other host notes:

- DSH's `bash` runs from the project root by default. Pass `workdir` when a
  command must run elsewhere, and prefer the wrapper's absolute path then.
- OpenSpec writes `openspec/` files; that is normal project work. Do not
  translate proposals/specs/tasks into `schemaJson` cards.
- The skills' "Project check" runs `openspec list --json` and reads `root`.
  With the wrapper this returns `{"root": {"path": ".../AlphaK", ...}}`.
- The project root is `/Users/ryan/Desktop/Project/AlphaK` and
  `openspec/config.yaml` uses the `spec-driven` schema.

## Skills installed

Project-scoped at `.dsh/skills/`:

- `openspec-propose`
- `openspec-explore`
- `openspec-apply-change`
- `openspec-update-change`
- `openspec-sync-specs`
- `openspec-archive-change`

Each `SKILL.md` now starts with a DSH host note that says to use
`.dsh/bin/openspec` and lists the slash-command mapping. Halmark at
`.dsh/skills/hallmark/` is unrelated.

## `/` menu shows no openspec skills

The Web composer's `/` menu fetches the skill catalog **once per session and
caches it**. Adding skill files while the page is open does not invalidate that
cache (only a plugin-manager change or a connection reset does), so a menu that
was warmed before the install keeps showing the old list.

Fix: reload the page (Cmd/Ctrl+R) or open a new session in the AlphaK
workspace, then type `/` — the six `openspec-*` entries appear. Typing the full
`/openspec-propose` still works without a refresh because the server resolves
the gesture against the live registry. Project-scoped skills only appear in
sessions whose working directory is AlphaK; a session in the default workspace
will not show them.

## Verification

```bash
.dsh/bin/openspec --version          # 1.14.0
.dsh/bin/openspec list --json        # root + changes
.dsh/bin/openspec context --json     # root.role = openspec_root
.dsh/bin/openspec schemas --json     # spec-driven
```

`node -v` in the same shell will still print v18 — that is expected; the
wrapper does not change PATH, it selects Node 20 for the CLI only.

## Updating OpenSpec

Install/upgrade with Node 20 so engine constraints are honoured:

```bash
PATH="$HOME/.nvm/versions/node/v20.20.2/bin:$PATH" \
  npm install -g @fission-ai/openspec
```

The wrapper picks up the CLI from the v20 global prefix automatically. If you
install it somewhere unusual, set `OPENSPEC_CLI`.

## Optional: fix bare `openspec` globally

The workspace fix is deliberate: it does not mutate your global npm bins. If
you also want `openspec` working in a normal terminal, either activate Node 20
before launching DSH (`nvm use 20`) or reinstall OpenSpec under Node 20 and
ensure that prefix is on PATH. Restart DSH afterwards so its inherited PATH
changes.

## Why not rewrite the skill commands

The skills contain dozens of literal `openspec ...` examples. Rather than
rewrite all of them (and break on the next `openspec update`), the host note
states once that every such command means `.dsh/bin/openspec`. That mirrors the
skills' existing `--store` shorthand convention and keeps re-sync simple.
