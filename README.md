# Hocuspocus-rs-ws

## Overview

Hocuspocus-rs-ws is a Rust implementation of the
[Hocuspocus collaborative editing protocol](https://github.com/ueberdosis/hocuspocus),
with a focus on the WebSocket workflow used by the current Hocuspocus v4
JavaScript provider. The core
synchronisation logic is derived from the open-source
[y-sweet project](https://github.com/y-sweet/y-sweet) to provide a solid
foundation for Yjs-powered document handling.

This crate deliberately targets a WebSocket + server runtime; it does not aim to
offer a fully transport-agnostic or Sans-IO abstraction.

Built atop Tokio and Rust's async/await ecosystem, the server is designed to
scale across multi-core, multi-threaded environments.

## Local Quickstart

Run the workspace example server alongside the Svelte demo client to test collaborative editing locally.

```bash
cargo run
```

The server listens on `http://0.0.0.0:3000`. In another terminal, launch the sample client:

```bash
cd examples/sample-client
bun install   # first run
bun run dev
```

Open the printed Vite dev URL (defaults to `http://localhost:5173`) and the client will connect to the local server automatically.

For a headless provider compatibility check, run:

```bash
cd examples/sample-client
bun run e2e:hocuspocus:server --require-session-awareness-sync
```

## Status

- Targets the WebSocket protocol shape used by `@hocuspocus/provider` 4.3.0,
  including v4 routing keys shaped as `documentName\0sessionId`.
- Keeps compatibility with older plain document-name frames that do not include
  a provider-version trailer.
- Ships with an in-memory store and a basic example server exposing the same
  handshake as the upstream JavaScript implementation.

## Version Alignment

The compatibility target is checked against the current npm `latest` versions as
of 2026-06-29:

- `@hocuspocus/provider` 4.3.0
- `@hocuspocus/common` 4.3.0
- `yjs` 13.6.31
- `y-protocols` 1.0.7

The sample client pins these lines through `package.json` and `bun.lock`, and
the Rust protocol tests cover the provider v4 auth, sync, awareness,
sync-status, ping, pong, close, and session-aware routing flows.

## Roadmap

- [x] Implement the baseline WebSocket handshake and sync loop.
- [x] Support the Hocuspocus provider v4 auth, sync-status, ping, pong, close,
      and `documentName\0sessionId` routing-key frames.
- [x] Provide an in-memory store backing documents with persistence hooks.
- [x] Expose a runnable example server for local development.
- [ ] Publish comparative benchmarks against the upstream JavaScript server.
- [ ] Expose callbacks for the full set of Hocuspocus event hooks.
- [ ] Provide additional store backends out of the box (filesystem, S3-compatible, and more).

## Attribution

This crate includes code adapted from the
[Hocuspocus JavaScript server](https://github.com/ueberdosis/hocuspocus) and the
[y-sweet project](https://github.com/y-sweet/y-sweet). Both upstream projects
are distributed under the MIT license, and the adapted portions in this
repository retain the original license terms. File headers highlight modules
that contain derivative work.

## License

Hocuspocus-rs-ws is released under the [MIT License](LICENSE). The license
notice applies to both original code and the portions derived from the upstream
projects listed above.
