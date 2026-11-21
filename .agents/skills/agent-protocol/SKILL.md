---
name: agent-protocol
description: Extended Agent Protocol v4.1 (§1–§7) for Krono. Load ONLY for architectural, multi-slice, cross-boundary or exploratory tasks (new modules/layers, schema/sync-protocol changes, >3 files, refactors of app.rs/UI signatures). Bounded tasks use only the GEMINI.md core block.
---

# Agent Protocol v4.1 — Extended (§1–§7)

### 1. Task Classes & Preflight Rules
- **`bounded`:** localized bugfix, feature, or refactor with clear boundaries. Output Preflight and proceed directly to implementation.
- **`architectural`:** cross-cutting changes, new modules/layers, schema/protocol shifts, or unclear boundaries. Output Preflight + concise design options and **wait for approval** before editing.
- **`exploratory`:** investigation, root-cause analysis, or auditing. Strict read-only mode until a verified hypothesis exists.

If the Edit Plan exceeds 3 files, persist the checklist to a working note/todo before editing.

### 2. Consent & Scope Control
**Proceed without asking:**
- the requested change plus strictly required imports, types, and unit/integration tests;
- read-only `graphify`, `tgrep`, `rtk`, LSP, file reads, git inspection, and test/build runs.

**Stop and ask first (4 Russian lines: `Что / Зачем / Цена / Риск`):**
- `architectural` shifts, unsolicited refactoring, new dependencies or abstraction layers;
- touching files outside the declared Edit Plan;
- deleting/renaming files, editing CI/lockfiles/secrets/shared test harnesses;
- updating docs/README unless public contract changed and docs are explicitly in scope (otherwise log in `Ideas`).

### 3. Discovery & Tooling Discipline (Krono-specific)
- **graphify** (graph at `graphify-out/graph.json`, never read the JSON/HTML directly):
  - `graphify query "<question>" --budget 800` — BFS context for a question.
  - `graphify explain "<Symbol>"` — node + neighbors.
  - `graphify affected "<Symbol>" --depth 2` — blast radius before changing a type/signature.
  - `graphify path "<A>" "<B>"` — how two symbols connect.
  - `graphify god-nodes --top 15` — hubs.
  - Freshness: after structural edits run `graphify update .` (AST only, free).
- **tgrep** (index at `.tgrep/`):
  - `tgrep -n -m5 -g "krono-*/**/*.rs" "<regex>"` — always scope with `-g`, always cap with `-m`.
  - `-l` for file list only, `-c` for counts, `-F` for literals, `-w` for whole words.
  - After big file moves: `tgrep index .`.
- **rtk** (log compression; all build/test/git output goes through it):
  - `rtk err cargo check --workspace` · `rtk test cargo test --workspace` · `rtk cargo clippy --workspace`
  - `rtk git status` · `rtk git log -10` · `rtk diff` · `rtk git diff --stat`
  - `rtk read <file>` only for skimming non-source files (logs, generated text).
- **Batching:** combine independent discovery calls, file reads, and edits into single parallel turns.
- **No blind re-reads:** never re-read a file after a successful edit unless a compiler/test points to a specific line.

### 4. Universal Architecture Invariants
- **Dependency Direction:** `krono-desktop (UI / storage / sync adapters) → krono-core`. `krono-core` must never depend on egui, rusqlite, ureq or the filesystem layout.
- **Boundary Validation:** parse and validate untrusted input (GitHub API, disk files, config JSON) at the outermost edge (`validate_rel_path`, serde models); pass validated domain structures inward.
- **Adapter Parity:** local disk scan, GitHub tree, watcher and SQLite index must accept the *same* set of vault files — one shared predicate in `krono-core`.
- **Safety & Hygiene:** parameterized SQL only, atomic file writes, LF-normalized content for hashing, no `unwrap`/`panic!` outside tests, no silent error swallowing.
- **Reuse Hierarchy:** `std → existing repo utilities → minimal local code → new crate (requires consent)`.

### 5. Adaptive Verification
- **Behavior / Bugfix / API change:** minimal stub + test first (RED on expected behavior) → implement → `rtk test cargo test -p <crate> <name>` → `rtk test cargo test --workspace` → `rtk err cargo check --workspace`.
- **Pure refactor / UI / config:** baseline `rtk err cargo check --workspace` → edits → same check + existing tests.
- Prefer core-level tests in `krono-core` (pure, fast) over UI-level tests.

### 6. Multi-Slice Orchestration (Large Tasks Only)
- Never spawn subagents for `bounded` or tightly coupled edits; execute directly in batched turns.
- For multi-slice tasks: lock shared contracts (types, `UiAction` variants, DTOs) on disk first, then assign strictly disjoint file sets (one writer per file).
- Subagents return compact summaries only (`changed files + diff stat + verification result`), never raw transcripts.

### 7. Stop Conditions & Output Format
Stop immediately and report in 4 Russian lines (`Сделано / Осталось / Почему стоп / Что нужно`) if:
- a needed command is blocked by sandbox/policy;
- real edits must go beyond the declared Edit Plan;
- the same check fails twice in a row with no new root-cause evidence;
- an unresolved ambiguity blocks safe implementation;
- plan state was lost after context compaction.

When verification is green and `git diff` is a subset of the Edit Plan, output strictly:

Changed: ...
Verified: ...
Left: ...
Ideas: ... (max 3: idea — benefit — cost; never implement unasked)
