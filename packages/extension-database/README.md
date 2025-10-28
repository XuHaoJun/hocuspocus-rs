# hocuspocus-extension-database

Database persistence extension for hocuspocus-rs (MVP). Provides a `DatabaseExtension` trait and a SQLite adapter using `sqlx`.

## Usage

```rust
use hocuspocus_extension_database::{DatabaseExtension, SqliteDatabase};
use hocuspocus_extension_database::types::{FetchContext, StoreContext};

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
