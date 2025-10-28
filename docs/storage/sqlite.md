## SQLite storage (sqlx)

### Schema (MVP)

```sql
CREATE TABLE IF NOT EXISTS documents (
  name TEXT PRIMARY KEY,
  state BLOB NOT NULL,
  updated_at INTEGER NOT NULL
);
```

- `name`: logical document identifier
- `state`: full encoded Yrs/Yjs document state (`Vec<u8>`)
- `updated_at`: unix epoch millis for last write

### Queries

- Upsert latest state:

```sql
INSERT INTO documents(name, state, updated_at)
VALUES(?, ?, ?)
ON CONFLICT(name) DO UPDATE SET
  state = excluded.state,
  updated_at = excluded.updated_at;
```

- Fetch latest state:

```sql
SELECT state FROM documents WHERE name = ?;
```

### Rust setup (docs)

Cargo features:

```toml
# Cargo.toml
sqlx = { version = "~0.7", features = ["runtime-tokio-rustls", "sqlite"] }
```

Pool and example operations:

```rust
use sqlx::{SqlitePool, Row};

pub async fn open_pool(url: &str) -> sqlx::Result<SqlitePool> {
    let pool = SqlitePool::connect(url).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS documents (name TEXT PRIMARY KEY, state BLOB NOT NULL, updated_at INTEGER NOT NULL)"
    ).execute(&pool).await?;
    Ok(pool)
}

pub async fn fetch_state(pool: &SqlitePool, name: &str) -> sqlx::Result<Option<Vec<u8>>> {
    if let Some(row) = sqlx::query("SELECT state FROM documents WHERE name = ?")
        .bind(name)
        .fetch_optional(pool)
        .await? {
        let bytes: Vec<u8> = row.get(0);
        Ok(Some(bytes))
    } else {
        Ok(None)
    }
}

pub async fn store_state(pool: &SqlitePool, name: &str, state: &[u8], updated_at: i64) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO documents(name, state, updated_at) VALUES(?, ?, ?)\n         ON CONFLICT(name) DO UPDATE SET state = excluded.state, updated_at = excluded.updated_at"
    )
        .bind(name)
        .bind(state)
        .bind(updated_at)
        .execute(pool)
        .await?;
    Ok(())
}
```

### Pragmas (optional)

- `PRAGMA journal_mode=WAL;` for better concurrency
- `PRAGMA synchronous=NORMAL;` balanced durability/throughput
