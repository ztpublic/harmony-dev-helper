# Harmony Dev Helper Monorepo

Shared webview architecture for:
- Tauri desktop app (Rust host)
- VSCode extension webview host
- IntelliJ plugin webview host

All hosts communicate with the same frontend over websocket (no Tauri commands).

## Stack

- `pnpm` workspaces + `turbo`
- React + TypeScript + Vite (shared webview)
- Tauri v2 + Rust
- Storybook (React components)

## Repo layout

- `apps/webview`: shared React UI used by all hosts
- `apps/desktop`: Tauri desktop shell + Rust websocket bridge
- `apps/vscode-extension`: VSCode host + websocket bridge
- `apps/intellij-plugin`: IntelliJ host + websocket bridge
- `packages/protocol`: shared bridge protocol types
- `packages/webview-bridge`: shared websocket client for the webview app

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
- Default behavior serves `src/main/resources/webview` assets (copied by `pnpm prepare:hosts`) via an embedded local HTTP server.
- Override with a live frontend URL using JVM option: `-Dharmony.webview.url=http://127.0.0.1:1420`.

## Bridge contract

Hosts inject one of:
- `window.__HARMONY_BRIDGE__ = { host, wsUrl }`
- or query params: `?host=intellij&wsUrl=ws://127.0.0.1:8789`

Frontend resolves bridge config in this order:
1. `window.__HARMONY_BRIDGE__`
2. `location.search` (`host`, `wsUrl`)
3. environment detection fallback (Tauri/VSCode/browser)

## Tauri HDC invoke actions

The desktop Rust backend vendors `hdckit-rs` under `apps/desktop/src-tauri/vendor/hdckit-rs` and exposes these websocket `invoke` actions:

- `hdc.listTargets` with `args: {}`
- `hdc.getParameters` with `args: { connectKey: string }`
- `hdc.shell` with `args: { connectKey: string, command: string }`
