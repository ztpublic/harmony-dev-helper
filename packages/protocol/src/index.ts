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
  "hdc.hilog.subscribe": boolean;
  "hdc.hilog.unsubscribe": boolean;
}

export type BinConfigSource = "custom" | "path" | "deveco" | "none";

export interface HdcBinConfigResult {
  customBinPath: string | null;
  resolvedBinPath: string | null;
  source: BinConfigSource;
  available: boolean;
  message?: string;
}

export interface HdcHilogSubscribeResult {
  subscriptionId: string;
  connectKey: string;
}

export interface HdcHilogUnsubscribeResult {
  stopped: boolean;
  subscriptionId?: string;
}

export interface HdcHilogBatchEventData {
  subscriptionId: string;
  connectKey: string;
  chunk: string;
  dropped: number;
}

export type HdcHilogState = "started" | "stopped" | "error";

export interface HdcHilogStateEventData {
  subscriptionId: string;
  connectKey: string;
  state: HdcHilogState;
  message?: string;
}

export type InvokeAction =
  | "host.getCapabilities"
  | "hdc.listTargets"
  | "hdc.getParameters"
  | "hdc.shell"
  | "hdc.getBinConfig"
  | "hdc.setBinPath"
  | "hdc.hilog.subscribe"
  | "hdc.hilog.unsubscribe";

export interface InvokeArgsByAction {
  "host.getCapabilities": Record<string, never>;
  "hdc.listTargets": Record<string, never>;
  "hdc.getParameters": { connectKey: string };
  "hdc.shell": { connectKey: string; command: string };
  "hdc.getBinConfig": Record<string, never>;
  "hdc.setBinPath": { binPath: string | null };
  "hdc.hilog.subscribe": { connectKey: string };
  "hdc.hilog.unsubscribe": { subscriptionId?: string };
}

export interface InvokeResultByAction {
  "host.getCapabilities": { capabilities: HostCapabilities };
  "hdc.listTargets": { targets: string[] };
  "hdc.getParameters": { parameters: Record<string, string> };
  "hdc.shell": { output: string };
  "hdc.getBinConfig": HdcBinConfigResult;
  "hdc.setBinPath": HdcBinConfigResult;
  "hdc.hilog.subscribe": HdcHilogSubscribeResult;
  "hdc.hilog.unsubscribe": HdcHilogUnsubscribeResult;
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

export type HilogEventPayload =
  | {
      name: "hdc.hilog.batch";
      data: HdcHilogBatchEventData;
    }
  | {
      name: "hdc.hilog.state";
      data: HdcHilogStateEventData;
    };

export type HostEventPayload =
  | InvokeEventPayload
  | HilogEventPayload
  | { name: string; data?: Record<string, unknown> };

export type HostMessage =
  | Envelope<"event", HostEventPayload>
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
