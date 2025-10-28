use anyhow::Result;
use async_trait::async_trait;
use hocuspocus_extension_database::types::{FetchContext, StoreContext};
use hocuspocus_extension_database::DatabaseExtension;
use sqlx::SqlitePool;

pub struct SqliteDatabase {
    pool: SqlitePool,
}

impl SqliteDatabase {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS documents (name TEXT PRIMARY KEY, state BLOB NOT NULL, updated_at INTEGER NOT NULL)"
        ).execute(&pool).await?;
        Ok(Self { pool })
    }

    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DatabaseExtension for SqliteDatabase {
    async fn fetch(&self, ctx: FetchContext) -> Result<Option<Vec<u8>>> {
        let bytes: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT state FROM documents WHERE name = ?")
                .bind(&ctx.document_name)
                .fetch_optional(&self.pool)
                .await?;
        Ok(bytes)
    }

    async fn store(&self, ctx: StoreContext<'_>) -> Result<()> {
        sqlx::query(
            "INSERT INTO documents(name, state, updated_at) VALUES(?, ?, ?)\n             ON CONFLICT(name) DO UPDATE SET state = excluded.state, updated_at = excluded.updated_at"
        )
            .bind(&ctx.document_name)
            .bind(ctx.state)
            .bind(ctx.updated_at_millis)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
