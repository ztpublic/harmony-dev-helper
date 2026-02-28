# Harmony Dev Helper Monorepo

Shared Harmony webview architecture for:
- Tauri desktop app
- VSCode extension
- IntelliJ plugin

All hosts use the same frontend and communicate with the backend over WebSocket (no Tauri `invoke` API path).

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
- Rust WebSocket bridge (`apps/hdc-bridge-rs`)
- Vendored `hdckit-rs` (`apps/desktop/src-tauri/vendor/hdckit-rs`)
- Tauri v2 desktop shell
- Storybook for webview component work

## Repository layout

- `apps/webview`: shared React UI used by all hosts
- `apps/hdc-bridge-rs`: shared Rust backend bridge (library + binary)
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

Starts the bridge in-process on `ws://127.0.0.1:8787`.

### VSCode extension

Build/watch extension TypeScript:

```bash
pnpm --filter @harmony/vscode-extension watch
```

Before testing packaged webview assets in VSCode, sync the built shared webview:

```bash
pnpm prepare:hosts
```

Notes:
- The shared UI is hosted in the bottom Panel area as the `Harmony` view container (`harmony.mainView`).
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

- Desktop: embedded bridge at `ws://127.0.0.1:8787`
- VSCode: sidecar bridge at `ws://127.0.0.1:8788`
- IntelliJ: sidecar bridge at `ws://127.0.0.1:8789`

### Sidecar startup order (VSCode + IntelliJ)

1. `HARMONY_HDC_BRIDGE_BIN` (absolute path to prebuilt `hdc-bridge-rs`)
2. Fallback:
   `cargo run --manifest-path apps/hdc-bridge-rs/Cargo.toml -- --ws-addr <host:port>`

Optional manifest override:
- `HARMONY_HDC_BRIDGE_MANIFEST_PATH`

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
