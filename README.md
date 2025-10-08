# Hocuspocus-rs-ws

## Overview

Hocuspocus-rs-ws is a Rust implementation of the
[Hocuspocus collaborative editing protocol](https://github.com/ueberdosis/hocuspocus),
with a focus on the WebSocket workflow shipped in Hocuspocus 3.2.4. The core
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

## Status

- Targets the baseline WebSocket feature set of Hocuspocus 3.2.4.
- Ships with an in-memory store and a basic example server exposing the same
  handshake as the upstream JavaScript implementation.

## Roadmap

- [x] Implement the baseline WebSocket handshake and sync loop from Hocuspocus 3.2.4.
- [x] Provide an in-memory store backing documents with persistence hooks.
- [x] Expose a runnable example server for local development.
- [ ] Support every extended message type emitted by the Hocuspocus provider.
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
