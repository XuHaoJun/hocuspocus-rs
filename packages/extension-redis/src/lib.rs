use anyhow::Result;
use redis::aio::ConnectionLike;
use redis::{AsyncCommands, Client, Msg};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::task::JoinHandle;
use bytes::Bytes;
use futures_util::stream::StreamExt;

#[derive(Clone)]
pub struct RedisBroadcaster {
    client: Client,
    instance_id: String,
}

impl RedisBroadcaster {
    pub async fn connect(url: &str, instance_id: String) -> Result<Self> {
        let client = Client::open(url)?;
        Ok(Self { client, instance_id })
    }

    fn chan_sync(doc: &str) -> String { format!("hpr:doc:{}:sync", doc) }
    fn chan_awareness(doc: &str) -> String { format!("hpr:doc:{}:awareness", doc) }

    pub async fn publish_sync(&self, doc: &str, payload: &[u8]) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        let mut buf = Vec::with_capacity(4 + self.instance_id.len() + payload.len());
        buf.extend((self.instance_id.len() as u32).to_be_bytes());
        buf.extend(self.instance_id.as_bytes());
        buf.extend_from_slice(payload);
        let _: () = conn.publish(Self::chan_sync(doc), buf).await?;
        Ok(())
    }

    pub async fn publish_awareness(&self, doc: &str, payload: &[u8]) -> Result<()> {
        let mut conn = self.client.get_async_connection().await?;
        let mut buf = Vec::with_capacity(4 + self.instance_id.len() + payload.len());
        buf.extend((self.instance_id.len() as u32).to_be_bytes());
        buf.extend(self.instance_id.as_bytes());
        buf.extend_from_slice(payload);
        let _: () = conn.publish(Self::chan_awareness(doc), buf).await?;
        Ok(())
    }

    pub async fn subscribe(
        &self,
        doc: String,
    ) -> Result<(JoinHandle<()>, UnboundedReceiver<(bool, Bytes)>)> {
        let con = self.client.get_async_connection().await?;
        let mut pubsub = con.into_pubsub();
        pubsub.subscribe(Self::chan_sync(&doc)).await?;
        pubsub.subscribe(Self::chan_awareness(&doc)).await?;
        let (tx, rx) = unbounded_channel();
        let instance_id = self.instance_id.clone();
        let handle = tokio::spawn(async move {
            let mut stream = pubsub.on_message();
            while let Some(msg) = stream.next().await {
                if let Ok(payload) = msg.get_payload::<Vec<u8>>() {
                    if payload.len() < 4 { continue; }
                    let len = u32::from_be_bytes([payload[0],payload[1],payload[2],payload[3]]) as usize;
                    if 4 + len > payload.len() { continue; }
                    let src = &payload[4..4+len];
                    if src == instance_id.as_bytes() { continue; }
                    let is_sync = msg.get_channel_name().ends_with(":sync");
                    let body = Bytes::copy_from_slice(&payload[4+len..]);
                    let _ = tx.send((is_sync, body));
                }
            }
        });
        Ok((handle, rx))
    }
}
