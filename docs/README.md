# Hocuspocus-rs

Rust port of the Hocuspocus collaboration backend focused on an MVP that implements only the database persistence extension. It uses `axum` for WebSocket transport and `yrs` for Yjs-compatible CRDT state.

## Status

- MVP focuses on persistence only (database extension)
- Storage: SQLite via `sqlx`
- Transport: `axum` WebSocket (skeleton planned in Phase 2)
- Auth: optional Hocuspocus handshake (disabled by default)

## Monorepo layout

- `packages/*` — new Rust code (mirrors `@hocuspocus` layout)
- `docs/*` — documentation
- `learn-projects/*` — read-only references (Hocuspocus TS, yrs-axum, yrs-warp, y-crdt)

## Quickstart

This repository is currently in the documentation phase of the MVP.

- Read `docs/mvp-scope.md` for the exact MVP boundaries
- Read `docs/extensions/database.md` for the database extension API
- Read `docs/storage/sqlite.md` for the schema and `sqlx` usage
- See `docs/architecture.md` for the broader system design

Implementation phases:

- Phase 0: Documentation and contributor rules
- Phase 1: `DatabaseExtension` trait + `SqliteDatabase` adapter (no server yet)
- Phase 2: `axum` WebSocket skeleton + lifecycle glue to call `fetch/store`

## Auth (optional)

When enabled, the server mirrors the Hocuspocus Auth handshake:

- Incoming first frame per document: `[varstring documentName][varuint 2][varuint 0][varstring token]`
- Success reply: `[varstring documentName][varuint 2][varuint 2][varstring "readonly"|"read-write"]`
- Deny reply: `[varstring documentName][varuint 2][varuint 1][varstring reason]`

Server configuration supports a pluggable `AuthProvider` with a built-in static token provider. By default, auth is disabled.

## Goals

- Mirror Hocuspocus database hooks in Rust
- Persist `yrs` document state as a single state blob per document (MVP)
- Keep extension stateless and idempotent; debounce handled at server layer

## Learn from submodules

- TypeScript Hocuspocus: `learn-projects/hocuspocus`
- Axum integration: `learn-projects/yrs-axum`
- Warp integration: `learn-projects/yrs-warp`
- Yrs/Yjs protocol and crates: `learn-projects/y-crdt`
