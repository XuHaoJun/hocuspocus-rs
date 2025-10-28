#![doc = "Database persistence extension for hocuspocus-rs (MVP)"]

pub mod types;

use anyhow::Result;
use async_trait::async_trait;
use types::{FetchContext, StoreContext};

#[async_trait]
pub trait DatabaseExtension: Send + Sync {
    async fn fetch(&self, ctx: FetchContext) -> Result<Option<Vec<u8>>>;
    async fn store(&self, ctx: StoreContext<'_>) -> Result<()>;
}
