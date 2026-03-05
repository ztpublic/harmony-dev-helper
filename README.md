# Harmony Dev Helper Monorepo

Shared Harmony webview architecture for:
- Tauri desktop app
- VSCode extension
- IntelliJ plugin

All hosts use the same frontend and communicate with `hdc-bridge-rs` over WebSocket. The same bridge process also serves an MCP Streamable HTTP endpoint.

## Current status (March 2026)

Implemented today:
- Shared React webview with two main tabs:
  - `Hilog`
  - `File Explorer`
- HDC target discovery + device selection (with friendly labels from device parameters when available).
- Hilog live console with:
  - start/stop via subscribe/unsubscribe
  - pause/resume
  - level filter (`D/I/W/E/F`)
  - PID filter (from `hdc.hilog.listPids`)
  - search with ANSI-safe highlighting
  - "Stick to end" toggle
  - terminal context menu (copy/select all/clear)
  - Ctrl/Cmd+click link handling for `http(s)` URLs and path-like tokens
  - optional "Add to Chat" action when host supports `ide.openChat`
- Device File Explorer with:
  - lazy tree loading over `hdc.fs.list`
  - absolute-path navigation
  - refresh/retry flows
  - recent expanded folders
  - right-click actions: copy path, upload to folder, download file
- Settings dialog for:
  - custom HDC binary path (manual input + host picker when supported)
  - Hilog history limit
  - app theme (dark/light)
- HDC binary config persistence at:
  - `~/.harmony-dev-helper/hdc-bridge.json`

Current limitation:
- MCP endpoint is infrastructure-only right now. Transport works, but HDC tool surface is not exposed yet.

## Stack

- `pnpm` workspaces + `turbo`
- React + TypeScript + Vite (`apps/webview`)
- Rust WebSocket + MCP HTTP bridge (`apps/hdc-bridge-rs`)
- Shared Rust HDC client crate (`apps/hdckit-rs`)
- Tauri v2 desktop host (`apps/desktop`)
- VSCode extension host (`apps/vscode-extension`)
- IntelliJ plugin host (`apps/intellij-plugin`)
- Storybook for webview components

## Repository layout

- `apps/webview`: shared React UI used by all hosts
- `apps/hdc-bridge-rs`: shared Rust backend bridge (library + binary)
- `apps/hdckit-rs`: shared Rust HDC client crate
- `apps/desktop`: Tauri shell; runs bridge in-process
- `apps/vscode-extension`: VSCode host; starts bridge sidecar
- `apps/intellij-plugin`: IntelliJ host; starts bridge sidecar
- `packages/protocol`: shared protocol types
- `packages/webview-bridge`: shared frontend WebSocket + host-bridge client
- `scripts/sync-webview-assets.mjs`: sync built webview into VSCode/IntelliJ host assets
- `scripts/sync-vscode-bridge-bin.mjs`: copy built bridge binary into VSCode extension

## Requirements

- Node.js + `pnpm` (workspace uses `pnpm@10.6.2`)
- Rust toolchain (`cargo`)
- IntelliJ plugin development: JDK 17

## Common commands

```bash
pnpm install

# Core development
pnpm dev:webview
pnpm dev:desktop
pnpm dev:vscode

# Host asset prep
pnpm prepare:hosts
pnpm prepare:vscode-host

# Quality/build
pnpm storybook
pnpm build
pnpm typecheck
pnpm lint
```

## Host development

### Desktop (Tauri)

```bash
pnpm dev:desktop
```

This runs Tauri with the shared webview dev server and starts bridge runtime in-process.

### VSCode extension

Compile extension in watch mode:

```bash
pnpm dev:vscode
```

Prepare host assets and bundled bridge binary:

```bash
pnpm prepare:vscode-host
```

Bridge sidecar startup order in VSCode:
1. `HARMONY_HDC_BRIDGE_BIN` (absolute path)
2. Bundled binary at `apps/vscode-extension/bin/hdc-bridge-rs` (or `.exe`)
3. Fallback `cargo run --manifest-path apps/hdc-bridge-rs/Cargo.toml -- --ws-addr 127.0.0.1:8788`

Optional manifest override:
- `HARMONY_HDC_BRIDGE_MANIFEST_PATH`

### IntelliJ plugin

1. Build webview and sync host assets:

```bash
pnpm prepare:hosts
```

2. Open `apps/intellij-plugin` in IntelliJ and run Gradle `runIde`.

Bridge sidecar startup order in IntelliJ:
1. `HARMONY_HDC_BRIDGE_BIN` (absolute path)
2. Fallback `cargo run --manifest-path <resolved>/apps/hdc-bridge-rs/Cargo.toml -- --ws-addr 127.0.0.1:8789`

Optional manifest override:
- `HARMONY_HDC_BRIDGE_MANIFEST_PATH`

By default IntelliJ serves `src/main/resources/webview` via:
- `http://127.0.0.1:8790/index.html`

You can override webview URL:
- `-Dharmony.webview.url=http://127.0.0.1:1420`

## Bridge runtime model

`apps/hdc-bridge-rs` is the single backend implementation for all hosts.

Default host addresses:
- Desktop:
  - WebSocket: `ws://127.0.0.1:8787`
  - MCP HTTP: `http://127.0.0.1:8887/mcp`
- VSCode:
  - WebSocket: `ws://127.0.0.1:8788`
  - MCP HTTP: `http://127.0.0.1:8888/mcp`
- IntelliJ:
  - WebSocket: `ws://127.0.0.1:8789`
  - MCP HTTP: `http://127.0.0.1:8889/mcp`

`hdc-bridge-rs` CLI:

```bash
hdc-bridge-rs [--ws-addr <host:port>] [--mcp-http-addr <host:port>]
```

When `--mcp-http-addr` is omitted, it is derived from WebSocket address:
- same host
- port = `ws_port + 100`

MCP endpoints:
- `POST|GET|DELETE /mcp` (Streamable HTTP transport)
- `GET /health` (returns `ok`)

## Runtime channel model

The webview uses two channels:
- Data plane (`hdc.*`): webview -> Rust bridge over WebSocket
- Control plane (`ide.*`): webview -> IDE/Tauri host bridge

Rust bridge owns HDC features. IDE-specific actions are handled by the host runtime.

## WebSocket protocol contract (current)

Envelope shape:
- `{ id, type, payload, ts }`

Supported invoke actions:
- `host.getCapabilities`
- `hdc.listTargets`
- `hdc.getParameters`
- `hdc.shell`
- `hdc.fs.list`
- `hdc.fs.upload`
- `hdc.fs.download`
- `hdc.getBinConfig`
- `hdc.setBinPath`
- `hdc.hilog.listPids`
- `hdc.hilog.subscribe`
- `hdc.hilog.unsubscribe`

Selected action args:
- `hdc.fs.list`: `{ connectKey, path, includeHidden? }` (`path` must be absolute device path)
- `hdc.fs.upload`: `{ connectKey, localPath, remoteDirectory }`
- `hdc.fs.download`: `{ connectKey, remotePath, localDirectory }`
- `hdc.hilog.subscribe`: `{ connectKey, level?, pid? }`
- `hdc.hilog.unsubscribe`: `{ subscriptionId? }`

Async bridge events:
- `${action}.result` for invoke responses
- `hdc.hilog.batch`
- `hdc.hilog.state`

## IDE host bridge contract (current)

Envelope shape:
- `{ channel, id, type, payload }`
- `channel` is always `harmony-host`
- `type` is `invoke | result | error`

Supported host actions:
- `ide.getCapabilities`
- `ide.getHostInfo`
- `ide.openFile`
- `ide.openPath`
- `ide.openExternal`
- `ide.openChat`
- `ide.openFilePicker`

### Host support matrix

- Tauri desktop:
  - `ide.openFilePicker`: supported
  - `ide.openFile`, `ide.openPath`, `ide.openExternal`, `ide.openChat`: unsupported
- VSCode extension:
  - All listed host actions supported
  - `ide.getHostInfo` detects `vscode` / `cursor` / `trae` by URI scheme and app name
- IntelliJ plugin:
  - `ide.getCapabilities`, `ide.getHostInfo`, `ide.openFile`: supported
  - `ide.openPath`, `ide.openExternal`, `ide.openChat`, `ide.openFilePicker`: intentionally no-op (`opened: false` or canceled picker)

Host bridge error codes:
- `UNSUPPORTED_HOST`
- `INVALID_ARGS`
- `FILE_NOT_FOUND`
- `OPEN_FAILED`
- `TIMEOUT`

## Frontend bootstrap contract

Hosts can provide one of:
- `window.__HARMONY_BRIDGE__ = { host, wsUrl }`
- query params: `?host=<host>&wsUrl=<ws-url>`

`@harmony/webview-bridge` resolution order:
1. `window.__HARMONY_BRIDGE__`
2. query params
3. environment fallback (`tauri` / `vscode` / `browser`)
