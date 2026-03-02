/// <reference types="vite/client" />

import type { BridgeBootstrap } from "@harmony/protocol";

declare global {
  interface VsCodeWebviewApi {
    postMessage: (message: unknown) => unknown;
  }

  interface Window {
    __HARMONY_BRIDGE__?: BridgeBootstrap;
    __TAURI__?: unknown;
    acquireVsCodeApi?: () => VsCodeWebviewApi;
    __HARMONY_INTELLIJ_HOST_INVOKE__?: (request: unknown) => Promise<unknown>;
  }
}

export {};
