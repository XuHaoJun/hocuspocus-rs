## Architecture

### Components
- Transport: `axum` WebSocket server
- CRDT: `yrs` (Yjs-compatible) document
- Hooks/Extensions: Rust traits mirroring Hocuspocus (MVP: persistence hooks only)
- Persistence: Database extension implementing `fetch` and `store`

### Lifecycle
1. Client connects via WebSocket to Axum route.
2. Server loads/creates a `yrs::Doc` for `documentName`.
3. `onLoadDocument`:
   - Database extension `fetch` returns last known state (optional).
   - Server applies state to the `yrs` doc.
4. Client syncs via y-sync messages (out of scope for MVP implementation, described here for context).
5. On changes, server debounces and calls `onStoreDocument`.
6. Database extension `store` persists the encoded state.

### Extension API (MVP)
```rust
#[async_trait::async_trait]
pub trait DatabaseExtension: Send + Sync {
    async fn fetch(&self, ctx: FetchContext) -> anyhow::Result<Option<Vec<u8>>>;
    async fn store(&self, ctx: StoreContext) -> anyhow::Result<()>;
}
```

Where `FetchContext`/`StoreContext` include: `document_name`, timestamp, and optionally request metadata if available.

### Debounce & Unload
- Debounce happens in the server layer to limit store frequency.
- Unload semantics can flush pending changes before drop; parity with Hocuspocus is a future step.

### Future Integration
- Awareness and broadcast: consider Redis for horizontal scale.
- Auth: bearer/JWT or hook-based; not in MVP.


