## Redis (future, optional)

Redis is not required for the MVP but is a strong candidate for:

- Presence/awareness fanout across nodes
- Pub/sub for broadcasting Y updates across processes
- Ephemeral coordination (locks, document ownership hints)

### Suggested channels
- `hpr:doc:{name}:updates` – publish compact updates for cross-node broadcast
- `hpr:doc:{name}:awareness` – presence signals

### Patterns
- Single-writer per doc (advisory) to reduce conflicts
- Use Redis streams or pub/sub depending on durability needs
- Keep storage of source-of-truth in SQL; Redis remains ephemeral

### Crates
- `redis` crate, async runtime compatible (Tokio)
