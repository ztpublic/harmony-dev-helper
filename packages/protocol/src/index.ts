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

export interface HostCapabilities {
  "host.getCapabilities": boolean;
  "hdc.listTargets": boolean;
  "hdc.getParameters": boolean;
  "hdc.shell": boolean;
  "hdc.getBinConfig": boolean;
  "hdc.setBinPath": boolean;
}

export type BinConfigSource = "custom" | "path" | "deveco" | "none";

export interface HdcBinConfigResult {
  customBinPath: string | null;
  resolvedBinPath: string | null;
  source: BinConfigSource;
  available: boolean;
  message?: string;
}

export type InvokeAction =
  | "host.getCapabilities"
  | "hdc.listTargets"
  | "hdc.getParameters"
  | "hdc.shell"
  | "hdc.getBinConfig"
  | "hdc.setBinPath";

export interface InvokeArgsByAction {
  "host.getCapabilities": Record<string, never>;
  "hdc.listTargets": Record<string, never>;
  "hdc.getParameters": { connectKey: string };
  "hdc.shell": { connectKey: string; command: string };
  "hdc.getBinConfig": Record<string, never>;
  "hdc.setBinPath": { binPath: string | null };
}

export interface InvokeResultByAction {
  "host.getCapabilities": { capabilities: HostCapabilities };
  "hdc.listTargets": { targets: string[] };
  "hdc.getParameters": { parameters: Record<string, string> };
  "hdc.shell": { output: string };
  "hdc.getBinConfig": HdcBinConfigResult;
  "hdc.setBinPath": HdcBinConfigResult;
}

type InvokePayload = {
  [A in InvokeAction]: {
    action: A;
    args: InvokeArgsByAction[A];
  };
}[InvokeAction];

export type ClientMessage = Envelope<"invoke", InvokePayload>;

type InvokeEventPayload = {
  [A in InvokeAction]: {
    name: `${A}.result`;
    data: InvokeResultByAction[A];
  };
}[InvokeAction];

export type HostMessage =
  | Envelope<"event", InvokeEventPayload | { name: string; data?: Record<string, unknown> }>
  | Envelope<"error", { code: string; message: string }>;

export function actionResultEventName<TAction extends InvokeAction>(
  action: TAction
): `${TAction}.result` {
  return `${action}.result` as `${TAction}.result`;
}

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
