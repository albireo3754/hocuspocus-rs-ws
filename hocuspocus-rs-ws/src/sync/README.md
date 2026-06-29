Fork of [y-sync](https://github.com/y-crdt/y-sync/tree/master) that runs fully-deterministically
in order to support wasm.

The WebSocket integration around this sync layer is tested against the current
Hocuspocus/Yjs npm line checked on 2026-06-29:

- `@hocuspocus/provider` 4.3.0
- `@hocuspocus/common` 4.3.0
- `yjs` 13.6.31
- `y-protocols` 1.0.7
