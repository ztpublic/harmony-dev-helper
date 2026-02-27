export type HarmonyHost = "browser" | "tauri" | "vscode" | "intellij";

export interface BridgeBootstrap {
  host: HarmonyHost;
  wsUrl: string;
  debug?: boolean;
}

export interface Envelope<TType extends string, TPayload> {
  id: string;
  type: TType;
  payload: TPayload;
  ts: number;
}

export type ClientMessage =
  | Envelope<"ping", { source: HarmonyHost; note?: string }>
  | Envelope<"invoke", { action: string; args?: Record<string, unknown> }>;

export type HostMessage =
  | Envelope<"pong", { host: HarmonyHost; note?: string }>
  | Envelope<"event", { name: string; data?: Record<string, unknown> }>
  | Envelope<"error", { code: string; message: string }>;

export function createEnvelope<TType extends string, TPayload>(
  type: TType,
  payload: TPayload,
  id = (() => {
    const maybeCrypto = (globalThis as { crypto?: { randomUUID?: () => string } }).crypto;
    return maybeCrypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  })()
): Envelope<TType, TPayload> {
  return {
    id,
    type,
    payload,
    ts: Date.now()
  };
}
