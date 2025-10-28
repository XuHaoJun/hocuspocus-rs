# Hocuspocus-rs MVP Documentation and Database Extension Plan

## Scope

- Port core concepts of `@hocuspocus/server` to Rust (Axum + Yrs), but implement only the Database extension for MVP.
- Storage defaults to SQLite via `sqlx`.
- No auth for MVP; development-mode only.
- Redis is noted for future pub/sub and presence; not required for MVP.

## Deliverables (docs + rules)

- `docs/README.md`: Project overview, quickstart, MVP status, roadmap.
- `docs/architecture.md`: Component map (Axum WS, Yrs doc, hooks, extensions), message flow.
- `docs/mvp-scope.md`: What’s in vs out, decisions (SQLite, no auth), future phases.
- `docs/extensions/database.md`: Database extension spec (fetch/store), lifecycle, error semantics, debouncing notes.
- `docs/storage/sqlite.md`: Schema, `sqlx` queries, migration strategy, WAL, durability tradeoffs.
- `docs/storage/redis.md`: Proposed usage for broadcast/presence, optional dependency notes.
- `docs/learn-projects.md`: Summary of `learn-projects/` repos (what to learn/borrow).
- `.cursorrules`: Contributor guidance for Cursor (directory map, priorities, constraints, code style, do/don’t list).

## Architecture (high level)

- Transport: Axum WebSocket server (learn from `learn-projects/yrs-axum/src/ws.rs`).
- CRDT: `yrs` (Yjs-compatible) for document state and updates.
- Hooks/Extensions: Rust traits mirroring Hocuspocus hooks; MVP exposes only the persistence hooks needed by the Database extension.
- Persistence: `onLoadDocument` applies last known state; `onStoreDocument` persists encoded doc state (debounced by server, not by the extension).

## API mapping (TS → Rust)

- TS Database extension:
  - `onLoadDocument(fetch)` → fetch latest state and `Y.applyUpdate`.
  - `onStoreDocument(store)` → persist `Y.encodeStateAsUpdate` of current doc.
- Rust Database trait (MVP):
  - `fetch(ctx) -> Option<Vec<u8>>` returns last known state.
  - `store(ctx) -> ()` persists encoded state blob.

Minimal trait sketch for docs:

```rust
#[async_trait::async_trait]
pub trait DatabaseExtension: Send + Sync {
    async fn fetch(&self, ctx: FetchContext) -> anyhow::Result<Option<Vec<u8>>>;
    async fn store(&self, ctx: StoreContext) -> anyhow::Result<()>;
}
```

Context structs (doc, name, headers/params as needed) mirror `onLoadDocumentPayload`/`onStoreDocumentPayload` fields where relevant.

## Data model (SQLite)

- Table `documents`:
  - `name TEXT PRIMARY KEY`
  - `state BLOB NOT NULL`
  - `updated_at INTEGER NOT NULL` (unix millis)
- Optional future table `updates` for incremental updates or audit (not in MVP).

## SQLx usage (SQLite)

- Pool setup using feature flags: `sqlx = { version = "~0.7", features = ["runtime-tokio-rustls", "sqlite"] }`.
- Queries:
  - Upsert on store: `INSERT INTO documents(name, state, updated_at) VALUES(?, ?, ?) ON CONFLICT(name) DO UPDATE SET state=excluded.state, updated_at=excluded.updated_at`.
  - Fetch latest: `SELECT state FROM documents WHERE name = ?`.
- Pragmas (docs only): enable WAL, synchronous=NORMAL for balanced durability.

## Debounce semantics

- Server layer debounces `onStoreDocument`; extension remains stateless and idempotent. Document immediately unloads only if configured; see `unloadImmediately` parity notes (documented for future parity).

## Error and recovery

- Fetch returning `None` creates a new document.
- Store failures bubble up and are logged; the server retries on next change (doc).
- Corrupt state handling: log and fall back to empty doc (doc-only guidance).

## Learnings from `learn-projects/`

- `learn-projects/hocuspocus`: 
  - Read API for `Database` (`fetch/store`) and hook payload shapes.
  - Message types and debouncing concepts in `server/src/types.ts`.
- `learn-projects/yrs-axum`: 
  - Axum WS upgrade, broadcast patterns, clean shutdown.
- `learn-projects/yrs-warp`: 
  - Alternative transport patterns, awareness ideas.
- `learn-projects/y-crdt`:
  - `yrs` crate structure; state encode/apply; test suites for behaviors.

## File map to reference in docs (future code, not implemented yet)

- `crates/server/` (axum runtime, hooks, doc manager) — outline only.
- `crates/extensions/database/` (trait + SQLite adapter) — outline only.

## Cursor rules (to write to `.cursorrules`)

Include:

- Project summary and goals.
- Priorities: implement database extension first; SQLite with sqlx; no auth; axum WS; respect submodules in `learn-projects/` (read-only).
- Directory map (`learn-projects/*` read-only reference; new Rust code under `crates/*`).
- Coding standards: explicit types on public APIs; early returns; avoid deep nesting; no empty catches; concise critical comments only.
- Do/Don’t:
  - Do: mirror TS hook semantics; keep extension stateless; use `yrs` for state encode/apply.
  - Don’t: change submodules; implement non-MVP packages; introduce auth.
- Operational: prefer absolute paths; keep lints clean; document schema changes.

## MVP Roadmap (phases)

- Phase 0 (now): Write the docs above and `.cursorrules`.
- Phase 1: Scaffold `DatabaseExtension` trait and a `SqliteDatabase` adapter (no server yet), plus a tiny test using an in-memory SQLite (`sqlite::memory:`).
- Phase 2: Axum WS skeleton and doc lifecycle glue to call `fetch/store` at load/store points.

## Files to create in this PR (docs only)

- `docs/README.md`
- `docs/architecture.md`
- `docs/mvp-scope.md`
- `docs/extensions/database.md`
- `docs/storage/sqlite.md`
- `docs/storage/redis.md`
- `docs/learn-projects.md`
- `.cursorrules`