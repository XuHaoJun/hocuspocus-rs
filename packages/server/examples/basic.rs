use axum::Router;
use hocuspocus_extension_database::SqliteDatabase;
use hocuspocus_server::{router, AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use dashmap::DashMap;
#[cfg(feature = "redis")]
use hocuspocus_extension_redis::RedisBroadcaster;

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
        #[cfg(feature = "redis")]
        redis: redis_opt,
    };

    let app: Router = router(state);

    let addr: SocketAddr = "127.0.0.1:4000".parse().unwrap();
    tracing::info!(%addr, "listening");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}
