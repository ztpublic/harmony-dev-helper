# Harmony Dev Helper Monorepo

Shared webview architecture for:
- Tauri desktop app
- VSCode extension webview host
- IntelliJ plugin webview host

All hosts communicate with the same frontend over websocket (no Tauri `invoke` commands).

## Stack

- `pnpm` workspaces + `turbo`
- React + TypeScript + Vite (shared webview)
- Rust websocket bridge + `hdckit-rs`
- Tauri v2 for desktop shell
- Storybook for UI components

## Repo layout

- `apps/webview`: shared React UI used by all hosts
- `apps/hdc-bridge-rs`: shared Rust websocket bridge (library + sidecar binary)
- `apps/desktop`: Tauri desktop shell using `hdc-bridge-rs` in-process
- `apps/vscode-extension`: VSCode host that launches `hdc-bridge-rs` sidecar
- `apps/intellij-plugin`: IntelliJ host that launches `hdc-bridge-rs` sidecar
- `packages/protocol`: shared websocket protocol/types
- `packages/webview-bridge`: shared websocket client + typed invoke helper

## Quick start

```bash
pnpm install
pnpm dev:webview
```

### Desktop (Tauri)

```bash
pnpm dev:desktop
```

### Storybook

```bash
pnpm storybook
```

### Plugin host assets

Build and copy webview bundle into plugin host resources:

```bash
pnpm prepare:hosts
```

### VSCode extension

```bash
pnpm --filter @harmony/vscode-extension build
```

### IntelliJ plugin

```bash
cd apps/intellij-plugin
# run the `runIde` Gradle task from IntelliJ's Gradle tool window
```

Notes:
- IntelliJ serves `src/main/resources/webview` assets (copied by `pnpm prepare:hosts`) via an embedded local HTTP server by default.
- Override IntelliJ webview URL with JVM option: `-Dharmony.webview.url=http://127.0.0.1:1420`.

## Shared Rust bridge model

`apps/hdc-bridge-rs` is the single HDC backend implementation.

- Desktop embeds it in-process and listens on `ws://127.0.0.1:8787`.
- VSCode launches it as a sidecar on `ws://127.0.0.1:8788`.
- IntelliJ launches it as a sidecar on `ws://127.0.0.1:8789`.

### Plugin-side sidecar startup (dev)

Plugin hosts start the bridge in this order:

1. `HARMONY_HDC_BRIDGE_BIN` (absolute path to prebuilt `hdc-bridge-rs` binary)
2. fallback: `cargo run --manifest-path apps/hdc-bridge-rs/Cargo.toml -- --ws-addr <host:port>`

Optional override for manifest lookup:
- `HARMONY_HDC_BRIDGE_MANIFEST_PATH`

Because the fallback uses Cargo, Rust toolchain is required in plugin-host development environments.

## Bridge contract

Hosts inject one of:
- `window.__HARMONY_BRIDGE__ = { host, wsUrl }`
- or query params: `?host=intellij&wsUrl=ws://127.0.0.1:8789`

Frontend resolves bridge config in this order:
1. `window.__HARMONY_BRIDGE__`
2. `location.search` (`host`, `wsUrl`)
3. environment fallback (Tauri/VSCode/browser)

### Typed invoke actions

The shared Rust bridge currently supports:

- `host.getCapabilities` with `args: {}`
- `hdc.listTargets` with `args: {}`
- `hdc.getParameters` with `args: { connectKey: string }`
- `hdc.shell` with `args: { connectKey: string, command: string }`

`host.getCapabilities` result includes:
- `host.getCapabilities`
- `hdc.listTargets`
- `hdc.getParameters`
- `hdc.shell`

All websocket envelopes remain:
- request/response fields: `id`, `type`, `payload`, `ts`
