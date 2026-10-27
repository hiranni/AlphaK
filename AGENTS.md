# AlphaK — agent notes

- Product requirements: `docs/requirements-analysis.md` (MVP: A-share market
  overview, stock detail, watchlist, charts; no full-market scanning).
- Implementation: `apps/api` is Rust + Axum + SQLite (demo seed); `apps/web` is
  Vite + React 19 + TypeScript with Hallmark tokens. Start with `README.md`.
- Agent-readable contracts: `docs/specs/` — JSON Schemas for strategy, strategy
  instance, agent profile, agent response, agent decision, and explainability cache; examples
  and a strategy-analysis template. Read order: `AGENTS.md` → requirements →
  `docs/specs/README.md` → `agent-response.schema.json` → `agent-decision.schema.json` → example.
- Hard invariants: at most 8 symbols per strategy; no full-market scan; no real
  orders; every Agent decision must pass schema + business validation; never
  let an Agent write to the order tables directly; strategy specs never declare
  an Agent — binding lives on the strategy instance.
- Explainability cache is analysis data, not NAV/P&L/trades. Default cache
  granularity is quarterly; cleanup may delete expired cache but never ledger
  records; replay is read-only.
- **OpenSpec is initialized here.** Always run the CLI as
  `.dsh/bin/openspec ...` — never bare `openspec`, which resolves to Node 18
  and crashes (OpenSpec requires Node >= 20.19). See `.dsh/OPENSPEC.md`.
- OpenSpec skills: `openspec-propose`, `openspec-explore`,
  `openspec-apply-change`, `openspec-update-change`, `openspec-sync-specs`,
  `openspec-archive-change`. DSH has no `/opsx` command; use the skill names.
- Design/UI work: load the `hallmark` skill (`.dsh/skills/hallmark/`). Its
  output is real project files, not `schemaJson` cards.

- Persistence changes MUST update `openspec/changes/data-retention-and-sqlite-performance/design.md`: lifecycle tables AND structural contract. Any table/column/index/constraint or retention-policy addition/change/removal requires `make persistence-doc-update`, manual review of retention/cleanup rules, and `make harness`. The harness probes a migrated in-memory DB and compares schema/policy fingerprints with the document. Build and CI enforce it. Never bypass it by deleting registrations or editing only fingerprints.
