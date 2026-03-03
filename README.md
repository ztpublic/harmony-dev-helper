# Harmony Dev Helper Monorepo

Shared Harmony webview architecture for:
- Tauri desktop app
- VSCode extension
- IntelliJ plugin

All hosts use the same frontend and communicate with the backend over WebSocket (no Tauri `invoke` API path). The same Rust backend process also exposes an MCP Streamable HTTP endpoint.

## Current state

The shared webview currently includes:
- HDC target discovery/selection
- Hilog live console (streaming, pause/resume, clear, auto-scroll, dropped-line counter)
- Settings dialog for:
  - HDC binary path (custom override + validation)
  - Hilog history limit
  - App theme (dark/light)

HDC binary config is persisted to:
- `~/.harmony-dev-helper/hdc-bridge.json`

## Stack

- `pnpm` workspaces + `turbo`
- React + TypeScript + Vite (`apps/webview`)
- Rust WebSocket + MCP Streamable HTTP bridge (`apps/hdc-bridge-rs`)
- Shared `hdckit-rs` crate (`apps/hdckit-rs`)
- Tauri v2 desktop shell
- Storybook for webview component work

## Repository layout

- `apps/webview`: shared React UI used by all hosts
- `apps/hdc-bridge-rs`: shared Rust backend bridge (library + binary)
- `apps/hdckit-rs`: shared Rust HDC client crate used by bridge/desktop flows
- `apps/desktop`: Tauri shell; embeds bridge in-process
- `apps/vscode-extension`: VSCode host; starts bridge sidecar
- `apps/intellij-plugin`: IntelliJ host; starts bridge sidecar
- `packages/protocol`: shared protocol types/envelopes
- `packages/webview-bridge`: shared browser WebSocket client + typed invoke helper

## Requirements

- Node.js + `pnpm` (workspace uses `pnpm@10`)
- Rust toolchain (`cargo`) for desktop and sidecar fallback flows
- IntelliJ plugin development: JDK 17

## Common commands

```bash
pnpm install
pnpm dev:webview
pnpm dev:desktop
pnpm dev:vscode
pnpm storybook
pnpm prepare:hosts
pnpm build
pnpm typecheck
pnpm lint
```

## Host development

### Desktop (Tauri)

```bash
pnpm dev:desktop
```

Starts the bridge in-process on:
- `ws://127.0.0.1:8787` (web frontend protocol)
- `http://127.0.0.1:8887/mcp` (MCP Streamable HTTP)
- `http://127.0.0.1:8887/health` (liveness)

### VSCode extension

Build/watch extension TypeScript:

```bash
pnpm --filter ./apps/vscode-extension watch
```

Prepare VSCode host assets (frontend + backend sidecar):

```bash
pnpm prepare:vscode-host
```

Notes:
- The shared UI is hosted in the bottom Panel area as the `Harmony` view container (`harmony-main-view`).
- The VSCode sidecar bridge starts lazily when the Harmony view is first opened.

### IntelliJ plugin

1. Build and sync webview assets:
```bash
pnpm prepare:hosts
```
2. Open `apps/intellij-plugin` in IntelliJ and run the `runIde` Gradle task.

Notes:
- The shared UI is hosted in the bottom `Harmony` tool window.
- The IntelliJ sidecar bridge starts when the tool window content is created.
- By default IntelliJ serves `src/main/resources/webview` via an embedded server (`http://127.0.0.1:8790`).
- You can override the URL with:
  `-Dharmony.webview.url=http://127.0.0.1:1420`

## Bridge runtime model

`apps/hdc-bridge-rs` is the single HDC backend implementation.

- Desktop:
  - WebSocket: `ws://127.0.0.1:8787`
  - MCP HTTP: `http://127.0.0.1:8887/mcp`
- VSCode:
  - WebSocket: `ws://127.0.0.1:8788`
  - MCP HTTP: `http://127.0.0.1:8888/mcp`
- IntelliJ:
  - WebSocket: `ws://127.0.0.1:8789`
  - MCP HTTP: `http://127.0.0.1:8889/mcp`

### MCP address derivation and override

By default, MCP HTTP bind address is derived from WebSocket bind address:
- host is preserved
- port = `ws_port + 100`

CLI options:
- `--ws-addr <host:port>`
- `--mcp-http-addr <host:port>` (optional override; when omitted, derived from `--ws-addr`)

MCP endpoints:
- `POST|GET|DELETE /mcp` (Streamable HTTP transport)
- `GET /health` (returns `ok`)

### Sidecar startup order

VSCode:
1. `HARMONY_HDC_BRIDGE_BIN` (absolute path to prebuilt `hdc-bridge-rs`)
2. Bundled binary (`apps/vscode-extension/bin/hdc-bridge-rs`) when present
3. Fallback: `cargo run --manifest-path apps/hdc-bridge-rs/Cargo.toml -- --ws-addr <host:port> [--mcp-http-addr <host:port>]`

IntelliJ:
1. `HARMONY_HDC_BRIDGE_BIN` (absolute path to prebuilt `hdc-bridge-rs`)
2. Fallback: `cargo run --manifest-path apps/hdc-bridge-rs/Cargo.toml -- --ws-addr <host:port> [--mcp-http-addr <host:port>]`

Optional manifest override:
- `HARMONY_HDC_BRIDGE_MANIFEST_PATH`

## Dual-channel runtime model

The shared webview now uses two channels:

- Data plane (`hdc.*`): webview -> Rust bridge over WebSocket (existing behavior)
- Control plane (`ide.*`): webview -> host IDE bridge (VSCode extension / IntelliJ plugin)

Rust remains responsible for HDC features only; IDE-specific actions are handled by the host runtime.

## Frontend bootstrap contract

Hosts provide one of:
- `window.__HARMONY_BRIDGE__ = { host, wsUrl }`
- query params: `?host=<host>&wsUrl=<ws-url>`

Resolution order in `@harmony/webview-bridge`:
1. `window.__HARMONY_BRIDGE__`
2. `location.search` (`host`, `wsUrl`)
3. environment fallback (`tauri`/`vscode`/`browser`)

## Protocol contract (current)

Envelope shape:
- `{ id, type, payload, ts }`

Supported invoke actions:
- `host.getCapabilities`
- `hdc.listTargets`
- `hdc.getParameters`
- `hdc.shell`
- `hdc.getBinConfig`
- `hdc.setBinPath`
- `hdc.hilog.subscribe`
- `hdc.hilog.unsubscribe`

`hdc.hilog.subscribe` args:
- `connectKey: string` (required)
- `level?: string` (optional, forwarded to `hilog -L`, e.g. `I,W,E` or `^D,I`)

Additional async host events:
- `hdc.hilog.batch`
- `hdc.hilog.state`

## IDE host bridge contract (new)

Host bridge envelope shape:
- `{ channel, id, type, payload }`
- `channel`: always `harmony-host`
- `type`: `invoke` | `result` | `error`

Supported invoke actions:
- `ide.getCapabilities`
- `ide.getHostInfo`
- `ide.openFile`
- `ide.openPath`
- `ide.openExternal`
- `ide.openChat`

`ide.getCapabilities` result:
- `{ capabilities: { "ide.openFile": boolean, "ide.openPath": boolean, "ide.openExternal": boolean, "ide.openChat": boolean } }`

`ide.getHostInfo` result:
- `{ host: { host: "vscode" | "cursor" | "trae" | "unknown", uriScheme: string, appName: string, isOfficialVsCode: boolean } }`
- VSCode extension uses `vscode.env.uriScheme` as primary detection and `vscode.env.appName` as fallback.

`ide.openFile` args:
- `path: string` (absolute filesystem path, required)
- `line?: number` (1-based, optional, default `1`)
- `column?: number` (1-based, optional, default `1`)
- `preview?: boolean` (optional, host best effort)
- `preserveFocus?: boolean` (optional, host best effort)

`ide.openFile` result:
- `{ opened: true }`

`ide.openPath` args:
- `path: string` (absolute or relative filesystem path, required)
- `line?: number` (1-based, optional)
- `column?: number` (1-based, optional)
- `preview?: boolean` (optional, host best effort for file opens)
- `preserveFocus?: boolean` (optional, host best effort for file opens)

`ide.openPath` result:
- `{ opened: boolean }`
- Open failures (unresolvable path, missing file, unsupported directory target) return `{ opened: false }` without raising host bridge errors.

`ide.openExternal` args:
- `url: string` (required, must be `http`/`https`)

`ide.openExternal` result:
- `{ opened: boolean }`
- Invalid URLs or external-open failures return `{ opened: false }` without raising host bridge errors.

`ide.openChat` args:
- `query: string` (required, chat input text)
- `isPartialQuery?: boolean` (optional, defaults to host behavior; for VSCode use `true` to fill input without sending)

`ide.openChat` result:
- `{ opened: boolean }`
- Unsupported hosts or open failures return `{ opened: false }`.

Host support (current):
- VSCode extension: `ide.getHostInfo`, `ide.openFile`, `ide.openPath`, `ide.openExternal`, `ide.openChat` all supported.
- IntelliJ plugin: `ide.getHostInfo` supported (returns `unknown` host); `ide.openFile` supported; `ide.openPath`, `ide.openExternal`, and `ide.openChat` are advertised as unsupported (`false`) and return no-op `{ opened: false }` if invoked.

Host bridge error codes:
- `UNSUPPORTED_HOST`
- `INVALID_ARGS`
- `FILE_NOT_FOUND`
- `OPEN_FAILED`
- `TIMEOUT`
