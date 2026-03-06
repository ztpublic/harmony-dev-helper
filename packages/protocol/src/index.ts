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
  "mcp.listTools": boolean;
  "hdc.listTargets": boolean;
  "hdc.getParameters": boolean;
  "hdc.shell": boolean;
  "hdc.fs.list": boolean;
  "hdc.fs.upload": boolean;
  "hdc.fs.download": boolean;
  "hdc.fs.downloadTemp": boolean;
  "hdc.fs.delete": boolean;
  "hdc.getBinConfig": boolean;
  "hdc.setBinPath": boolean;
  "hdc.hilog.listPids": boolean;
  "hdc.hilog.subscribe": boolean;
  "hdc.hilog.unsubscribe": boolean;
}

export interface IdeCapabilities {
  "ide.openFile": boolean;
  "ide.openPath": boolean;
  "ide.openExternal": boolean;
  "ide.openChat": boolean;
  "ide.openFilePicker": boolean;
  "ide.cursorMcp.addServer": boolean;
  "ide.cursorMcp.removeServer": boolean;
}

export type IdeHost = "vscode" | "cursor" | "trae" | "unknown";

export interface IdeHostInfo {
  host: IdeHost;
  uriScheme: string;
  appName: string;
  isOfficialVsCode: boolean;
}

export interface IdeOpenFileArgs {
  path: string;
  line?: number;
  column?: number;
  preview?: boolean;
  preserveFocus?: boolean;
}

export interface IdeOpenPathArgs {
  path: string;
  line?: number;
  column?: number;
  preview?: boolean;
  preserveFocus?: boolean;
}

export interface IdeOpenExternalArgs {
  url: string;
}

export interface IdeOpenChatArgs {
  query: string;
  isPartialQuery?: boolean;
}

export interface IdeOpenFilePickerArgs {
  canSelectFiles?: boolean;
  canSelectFolders?: boolean;
  canSelectMany?: boolean;
  title?: string;
  defaultPath?: string;
  filters?: Record<string, string[]>;
}

export interface IdeCursorMcpStatusResult {
  added: boolean;
}

export type IdeInvokeAction =
  | "ide.getCapabilities"
  | "ide.getHostInfo"
  | "ide.openFile"
  | "ide.openPath"
  | "ide.openExternal"
  | "ide.openChat"
  | "ide.openFilePicker"
  | "ide.cursorMcp.addServer"
  | "ide.cursorMcp.removeServer";

export interface IdeInvokeArgsByAction {
  "ide.getCapabilities": Record<string, never>;
  "ide.getHostInfo": Record<string, never>;
  "ide.openFile": IdeOpenFileArgs;
  "ide.openPath": IdeOpenPathArgs;
  "ide.openExternal": IdeOpenExternalArgs;
  "ide.openChat": IdeOpenChatArgs;
  "ide.openFilePicker": IdeOpenFilePickerArgs;
  "ide.cursorMcp.addServer": Record<string, never>;
  "ide.cursorMcp.removeServer": Record<string, never>;
}

export interface IdeInvokeResultByAction {
  "ide.getCapabilities": { capabilities: IdeCapabilities };
  "ide.getHostInfo": { host: IdeHostInfo };
  "ide.openFile": { opened: true };
  "ide.openPath": { opened: boolean };
  "ide.openExternal": { opened: boolean };
  "ide.openChat": { opened: boolean };
  "ide.openFilePicker": { canceled: boolean; paths: string[] };
  "ide.cursorMcp.addServer": IdeCursorMcpStatusResult;
  "ide.cursorMcp.removeServer": IdeCursorMcpStatusResult;
}

type IdeInvokePayload = {
  [A in IdeInvokeAction]: {
    action: A;
    args: IdeInvokeArgsByAction[A];
  };
}[IdeInvokeAction];

type IdeResultPayload = {
  [A in IdeInvokeAction]: {
    action: A;
    data: IdeInvokeResultByAction[A];
  };
}[IdeInvokeAction];

type IdeErrorPayload = {
  [A in IdeInvokeAction]: {
    action: A;
    code: string;
    message: string;
  };
}[IdeInvokeAction];

export type HostBridgeInvokeMessage = {
  channel: "harmony-host";
  id: string;
  type: "invoke";
  payload: IdeInvokePayload;
};

export type HostBridgeResultMessage = {
  channel: "harmony-host";
  id: string;
  type: "result";
  payload: IdeResultPayload;
};

export type HostBridgeErrorMessage = {
  channel: "harmony-host";
  id: string;
  type: "error";
  payload: IdeErrorPayload;
};

export type HostBridgeMessage =
  | HostBridgeInvokeMessage
  | HostBridgeResultMessage
  | HostBridgeErrorMessage;

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

export interface HdcHilogPidOption {
  pid: number;
  command: string;
}

export interface HdcHilogListPidsResult {
  pids: HdcHilogPidOption[];
}

export interface HdcFsListEntry {
  path: string;
  name: string;
  kind: "directory" | "file";
}

export interface HdcFsListResult {
  entries: HdcFsListEntry[];
}

export interface HdcFsUploadResult {
  remotePath: string;
}

export interface HdcFsDownloadResult {
  localPath: string;
}

export interface HdcFsDownloadTempResult {
  localPath: string;
  byteLength: number;
}

export interface HdcFsDeleteResult {
  deletedPath: string;
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

export interface McpToolSummary {
  name: string;
  title?: string;
  description?: string;
}

export type InvokeAction =
  | "host.getCapabilities"
  | "mcp.listTools"
  | "hdc.listTargets"
  | "hdc.getParameters"
  | "hdc.shell"
  | "hdc.fs.list"
  | "hdc.fs.upload"
  | "hdc.fs.download"
  | "hdc.fs.downloadTemp"
  | "hdc.fs.delete"
  | "hdc.getBinConfig"
  | "hdc.setBinPath"
  | "hdc.hilog.listPids"
  | "hdc.hilog.subscribe"
  | "hdc.hilog.unsubscribe";

export interface InvokeArgsByAction {
  "host.getCapabilities": Record<string, never>;
  "mcp.listTools": Record<string, never>;
  "hdc.listTargets": Record<string, never>;
  "hdc.getParameters": { connectKey: string };
  "hdc.shell": { connectKey: string; command: string };
  "hdc.fs.list": { connectKey: string; path: string; includeHidden?: boolean };
  "hdc.fs.upload": { connectKey: string; localPath: string; remoteDirectory: string };
  "hdc.fs.download": { connectKey: string; remotePath: string; localDirectory: string };
  "hdc.fs.downloadTemp": { connectKey: string; remotePath: string; maxBytes?: number };
  "hdc.fs.delete": { connectKey: string; path: string };
  "hdc.getBinConfig": Record<string, never>;
  "hdc.setBinPath": { binPath: string | null };
  "hdc.hilog.listPids": { connectKey: string };
  "hdc.hilog.subscribe": { connectKey: string; level?: string; pid?: number };
  "hdc.hilog.unsubscribe": { subscriptionId?: string };
}

export interface InvokeResultByAction {
  "host.getCapabilities": { capabilities: HostCapabilities };
  "mcp.listTools": { tools: McpToolSummary[] };
  "hdc.listTargets": { targets: string[] };
  "hdc.getParameters": { parameters: Record<string, string> };
  "hdc.shell": { output: string };
  "hdc.fs.list": HdcFsListResult;
  "hdc.fs.upload": HdcFsUploadResult;
  "hdc.fs.download": HdcFsDownloadResult;
  "hdc.fs.downloadTemp": HdcFsDownloadTempResult;
  "hdc.fs.delete": HdcFsDeleteResult;
  "hdc.getBinConfig": HdcBinConfigResult;
  "hdc.setBinPath": HdcBinConfigResult;
  "hdc.hilog.listPids": HdcHilogListPidsResult;
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
