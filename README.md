# hocuspocus-rs

Rust MVP of Hocuspocus-like server-side persistence built on yrs and axum. The MVP focuses on the database extension only, providing a stateless database extension trait and a SQLite adapter. The server debounces store operations and persists full state blobs (no incremental updates in MVP). Auth via the Hocuspocus handshake is supported but disabled by default.

### What’s here

- **Server (`packages/server`)**: Axum WebSocket server that speaks the Hocuspocus wire framing over yrs, debounces writes, and integrates database extensions.
- **SQLite database extension (`packages/extension-sqlite`)**: `sqlx`-based adapter implementing the database extension trait.
- Optional Redis broadcaster behind the `redis` feature for multi-instance fan-out (future-friendly; optional for MVP).

### Quick start

Server example (Axum, listens on `ws://127.0.0.1:4000/ws`):

```rust
use axum::routing::get;
use axum::Router;
use dashmap::DashMap;
#[cfg(feature = "redis")]
use hocuspocus_extension_redis::RedisBroadcaster;
use hocuspocus_extension_sqlite::SqliteDatabase;
use hocuspocus_server::{ws_handler, AppState, AuthScope, StaticTokenAuth};
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let db = SqliteDatabase::connect("sqlite::memory:").await?;

    #[cfg(feature = "redis")]
    let redis_opt = {
        if let Ok(url) = std::env::var("REDIS_URL") {
            let iid = std::env::var("INSTANCE_ID")
                .unwrap_or_else(|_| format!("instance-{}", std::process::id()));
            tracing::info!(url = %url, iid = %iid, "connecting to redis");
            let bc = RedisBroadcaster::connect(&url, iid).await?;
            Some(Arc::new(bc))
        } else {
            tracing::warn!("REDIS_URL not set; redis will not be used");
            None
        }
    };

    let state = AppState {
        db: Arc::new(db),
        debounce_ms: 250,
        max_debounce_ms: 2000,
        doc_counts: DashMap::new(),
        doc_latest: DashMap::new(),
        auth: Some(Arc::new(StaticTokenAuth {
            token: "test".to_string(),
            scope: AuthScope::ReadWrite,
        })),
        #[cfg(feature = "redis")]
        redis: redis_opt,
    };

    let app: Router = Router::new()
        .route(
            "/ws",
            get(|ws, state| ws_handler::<SqliteDatabase>(ws, state)),
        )
        .with_state(Arc::new(state));

    let addr: SocketAddr = "127.0.0.1:4000".parse().unwrap();
    tracing::info!(%addr, "listening");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
```

Frontend example (Provider):

```javascript
import * as Y from "yjs";
import { HocuspocusProvider } from "@hocuspocus/provider";

// Connect it to the backend
const provider = new HocuspocusProvider({
  url: "ws://127.0.0.1:4000/ws",
  name: "example-document1",
  token: "test",
});

// Define `tasks` as an Array
const tasks = provider.document.getArray("tasks");

// Listen for changes
tasks.observe((event) => {
  console.log(event.delta);
  console.log(provider.document.toJSON());
  console.log("tasks were modified");
});

// Add a new task
console.log("Adding new task");
tasks.push(["buy milk"]);
```

- **Auth (optional)**: The example enables a static token for demonstration. Use token `test` in your client’s Hocuspocus auth handshake (e.g., Provider `token: 'test'`).
- **Persist to disk**: Change the example’s `SqliteDatabase::connect("sqlite::memory:")` to a file URL like `sqlite:////tmp/hocus.db` if you want durable storage.

### Redis (optional)

Build and run the example with Redis support and environment configured:

```bash
REDIS_URL=redis://127.0.0.1:6379 INSTANCE_ID=instance-1 \
cargo run -p hocuspocus-server --features redis --example basic
```

### Notes

- The extension is stateless; the server handles debounce and stores full document state blobs only (MVP).
- Auth is Hocuspocus-handshake compatible; when disabled, the server accepts messages without authentication.

### License

MIT
