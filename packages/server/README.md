# hocuspocus-server (MVP skeleton)

Axum WebSocket skeleton that wires `DatabaseExtension.fetch/store` to a `yrs::Doc`.

## Example

```bash
cargo run --example basic -p hocuspocus-server
```

Then connect a Yjs provider to `ws://127.0.0.1:4000/ws/default`.
