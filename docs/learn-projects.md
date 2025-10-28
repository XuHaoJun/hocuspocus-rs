# Learn projects summary

## Hocuspocus (TypeScript)
- Plug & play collaboration backend for Yjs.
- Database extension exposes `fetch`/`store` used by `onLoadDocument`/`onStoreDocument`.
- Hook payloads and message types in `packages/server/src/types.ts` inform Rust API shapes.

## yrs-axum (Rust)
- Axum-based WebSocket integration for Yrs/Yjs protocol.
- BroadcastGroup pattern to gossip updates among peers in a room.
- Provides a signaling service for y-webrtc.

## yrs-warp (Rust)
- Warp-based equivalent integration.
- Similar broadcast pattern; alternative server framework reference.

## y-crdt (Rust)
- Core `yrs` crate implementing Yjs-compatible CRDT.
- `ywasm` and `yffi` wrappers for other runtimes.
- Reference for state encode/apply behavior and protocol compatibility.

## What we borrow
- Extension semantics (`fetch/store`) from Hocuspocus.
- Axum WS handler and broadcast patterns from `yrs-axum`.
- CRDT state operations from `yrs`.
- Warp-based patterns from `yrs-warp` (alternative reference).
