/// <reference types="vite/client" />

import type { BridgeBootstrap } from "@harmony/protocol";

declare global {
  interface Window {
    __HARMONY_BRIDGE__?: BridgeBootstrap;
    __TAURI__?: unknown;
    acquireVsCodeApi?: () => unknown;
  }
}

export {};
