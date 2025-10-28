## MVP Scope

### In scope
- Database extension trait and SQLite adapter using `sqlx`
- Apply latest stored state on load; persist full encoded state on store
- Axum-based WebSocket scaffolding (skeleton in Phase 2)
 - Optional Auth handshake (disabled by default)

### Out of scope
- Authorization models beyond the handshake (RBAC/ABAC, scopes beyond readonly/read-write)
- Redis presence/pub-sub
- Non-database extensions and advanced hooks
- Incremental update storage or audit trails
- Provider/y-sync/awareness protocol compatibility (future phase)

### Decisions
- Database: SQLite via `sqlx`
- Auth: optional Hocuspocus handshake (Token → Authenticated/PermissionDenied), disabled by default
- Debounce: handled by server; extension remains stateless

### Roadmap
- Phase 0: Docs and contributor rules
- Phase 1: Database trait + SQLite adapter (in-memory test)
- Phase 2: Axum WS skeleton + lifecycle glue


