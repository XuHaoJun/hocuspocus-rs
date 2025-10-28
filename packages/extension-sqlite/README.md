# hocuspocus-extension-sqlite

SQLite adapter for the `hocuspocus-rs` database extension. Provides an implementation of `DatabaseExtension` backed by SQLite using `sqlx`.

## Usage

```rust
use hocuspocus_extension_sqlite::SqliteDatabase;
use hocuspocus_extension_database::{DatabaseExtension, types::{FetchContext, StoreContext}};

# async fn demo() -> anyhow::Result<()> {
let db = SqliteDatabase::connect("sqlite::memory:").await?;

// fetch
let bytes = db.fetch(FetchContext { document_name: "doc".into() }).await?;

// store
db.store(StoreContext { document_name: "doc".into(), state: b"...", updated_at_millis: 0 }).await?;
# Ok(())
# }
```

Table schema is created automatically as `documents(name TEXT PRIMARY KEY, state BLOB NOT NULL, updated_at INTEGER NOT NULL)`.
