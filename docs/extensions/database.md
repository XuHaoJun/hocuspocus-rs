## Database Extension (MVP)

Implements persistence for `yrs` documents by fetching the latest state on load and storing the current state on changes.

### Trait
```rust
#[async_trait::async_trait]
pub trait DatabaseExtension: Send + Sync {
    async fn fetch(&self, ctx: FetchContext) -> anyhow::Result<Option<Vec<u8>>>;
    async fn store(&self, ctx: StoreContext) -> anyhow::Result<()>;
}
```

### Contexts
- FetchContext: { document_name: String }
- StoreContext: { document_name: String, state: Vec<u8>, updated_at_millis: i64 }

### Semantics
- fetch: return `Ok(None)` if the document does not exist.
- store: upsert latest full state; idempotent.
- errors: bubble up; server logs and retries on next change.

### Debouncing
- Server debounces `store` calls. The extension does no internal batching.

### Concurrency
- Last-write-wins at the state blob level (MVP). Future: incremental updates or CRDT merge at storage.

### Parity with TypeScript
- Mirrors `@hocuspocus/extension-database` (`fetch/store` and `onLoadDocument`/`onStoreDocument`).


