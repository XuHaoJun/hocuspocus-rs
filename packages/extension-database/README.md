# hocuspocus-extension-database

Database persistence extension for hocuspocus-rs (MVP). Provides the `DatabaseExtension` trait and context types.

## Usage

For a SQLite implementation, use the `hocuspocus-extension-sqlite` crate.

```rust
use hocuspocus_extension_database::{DatabaseExtension, types::{FetchContext, StoreContext}};
use hocuspocus_extension_sqlite::SqliteDatabase;

# async fn demo() -> anyhow::Result<()> {
let db = SqliteDatabase::connect("sqlite::memory:").await?;

// fetch (may be None)
let state = db.fetch(FetchContext { document_name: "doc1".into() }).await?;

// store
let state_bytes = vec![1,2,3];
db.store(StoreContext {
    document_name: "doc1".into(),
    state: &state_bytes,
    updated_at_millis: 0,
}).await?;
# Ok(())
# }
```
