import {
  actionResultEventName,
  createEnvelope,
  type BridgeBootstrap,
  type ClientMessage,
  type HarmonyHost,
  type HostBridgeErrorMessage,
  type HostBridgeInvokeMessage,
  type HostBridgeMessage,
  type HostBridgeResultMessage,
  type HostMessage,
  type IdeCapabilities,
  type IdeInvokeAction,
  type IdeInvokeArgsByAction,
  type IdeInvokeResultByAction,
  type InvokeAction,
  type InvokeArgsByAction,
  type InvokeResultByAction
} from "@harmony/protocol";

export type ConnectionState = "idle" | "connecting" | "open" | "closed" | "error";

type StatusListener = (state: ConnectionState) => void;
type MessageListener = (message: HostMessage) => void;

type HostBridgeErrorCode =
  | "UNSUPPORTED_HOST"
  | "INVALID_ARGS"
  | "FILE_NOT_FOUND"
  | "OPEN_FAILED"
  | "TIMEOUT";

interface PendingInvoke {
  action: InvokeAction;
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  timeoutHandle: ReturnType<typeof setTimeout>;
}

interface PendingHostInvoke {
  action: IdeInvokeAction;
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  timeoutHandle: ReturnType<typeof setTimeout>;
}

interface LocationLike {
  search: string;
}

interface VsCodeWebviewApi {
  postMessage: (message: unknown) => unknown;
}

interface WindowLike {
  __HARMONY_BRIDGE__?: BridgeBootstrap;
  __TAURI__?: unknown;
  acquireVsCodeApi?: () => VsCodeWebviewApi;
  __HARMONY_INTELLIJ_HOST_INVOKE__?: (
    request: HostBridgeInvokeMessage
  ) => Promise<HostBridgeResultMessage | HostBridgeErrorMessage | unknown>;
  location?: LocationLike;
  addEventListener?: (type: "message", listener: (event: MessageEvent<unknown>) => void) => void;
  removeEventListener?: (type: "message", listener: (event: MessageEvent<unknown>) => void) => void;
}

export interface InvokeOptions {
  timeoutMs?: number;
}

const DEFAULT_INVOKE_TIMEOUT_MS = 8_000;

function normalizeHostBridgeErrorCode(code: unknown): HostBridgeErrorCode {
  switch (code) {
    case "UNSUPPORTED_HOST":
    case "INVALID_ARGS":
    case "FILE_NOT_FOUND":
    case "OPEN_FAILED":
    case "TIMEOUT":
      return code;
    default:
      return "OPEN_FAILED";
  }
}

function toHostBridgeError(code: HostBridgeErrorCode, message: string): Error {
  return new Error(`[${code}] ${message}`);
}

function randomId(prefix: string): string {
  const maybeCrypto = (globalThis as { crypto?: { randomUUID?: () => string } }).crypto;
  return `${prefix}-${maybeCrypto?.randomUUID?.() ?? `${Date.now()}-${Math.random().toString(16).slice(2)}`}`;
}

function parseQueryBootstrap(locationLike?: LocationLike): BridgeBootstrap | undefined {
  if (!locationLike) {
    return undefined;
  }

  const params = new URLSearchParams(locationLike.search);
  const wsUrl = params.get("wsUrl");
  const host = params.get("host") as HarmonyHost | null;

  if (!wsUrl || !host) {
    return undefined;
  }

  return { host, wsUrl };
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isIdeInvokeAction(value: unknown): value is IdeInvokeAction {
  return (
    value === "ide.getCapabilities" ||
    value === "ide.openFile" ||
    value === "ide.openPath" ||
    value === "ide.openExternal"
  );
}

function isHostBridgeMessage(value: unknown): value is HostBridgeMessage {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (value.channel !== "harmony-host" || typeof value.id !== "string") {
    return false;
  }

  if (value.type !== "invoke" && value.type !== "result" && value.type !== "error") {
    return false;
  }

  const payload = value.payload;
  if (!isObjectRecord(payload) || !isIdeInvokeAction(payload.action)) {
    return false;
  }

  if (value.type === "invoke") {
    return isObjectRecord(payload.args);
  }

  if (value.type === "result") {
    return isObjectRecord(payload.data);
  }

  return typeof payload.code === "string" && typeof payload.message === "string";
}

function isHostBridgeResultOrError(
  value: unknown
): value is HostBridgeResultMessage | HostBridgeErrorMessage {
  return isHostBridgeMessage(value) && (value.type === "result" || value.type === "error");
}

function createHostBridgeInvokeMessage<TAction extends IdeInvokeAction>(
  action: TAction,
  args: IdeInvokeArgsByAction[TAction]
): HostBridgeInvokeMessage {
  return {
    channel: "harmony-host",
    id: randomId("host-invoke"),
    type: "invoke",
    payload: {
      action,
      args
    } as HostBridgeInvokeMessage["payload"]
  };
}

export function resolveBootstrap(windowLike: WindowLike = globalThis as unknown as WindowLike): BridgeBootstrap {
  if (windowLike.__HARMONY_BRIDGE__) {
    return windowLike.__HARMONY_BRIDGE__;
  }

  const queryBridge = parseQueryBootstrap(windowLike.location);
  if (queryBridge) {
    return queryBridge;
  }

  if (windowLike.__TAURI__) {
    return { host: "tauri", wsUrl: "ws://127.0.0.1:8787" };
  }

  if (typeof windowLike.acquireVsCodeApi === "function") {
    return { host: "vscode", wsUrl: "ws://127.0.0.1:8788" };
  }

  return { host: "browser", wsUrl: "ws://127.0.0.1:8787" };
}

export class HarmonyHostBridgeClient {
  private readonly bootstrap: BridgeBootstrap;
  private readonly windowLike: WindowLike;
  private readonly pendingInvokes = new Map<string, PendingHostInvoke>();
  private isVsCodeMessageListenerAttached = false;
  private vsCodeApi: VsCodeWebviewApi | null | undefined;

  private readonly onVsCodeMessage = (event: MessageEvent<unknown>): void => {
    const message = event.data;
    if (!isHostBridgeResultOrError(message)) {
      return;
    }

    const pending = this.pendingInvokes.get(message.id);
    if (!pending) {
      return;
    }

    clearTimeout(pending.timeoutHandle);
    this.pendingInvokes.delete(message.id);

    if (message.payload.action !== pending.action) {
      pending.reject(
        toHostBridgeError(
          "OPEN_FAILED",
          `Unexpected host bridge action: expected ${pending.action}, got ${message.payload.action}`
        )
      );
      return;
    }

    if (message.type === "error") {
      pending.reject(
        toHostBridgeError(normalizeHostBridgeErrorCode(message.payload.code), message.payload.message)
      );
      return;
    }

    pending.resolve(message.payload.data ?? {});
  };

  constructor(
    bootstrap: BridgeBootstrap,
    windowLike: WindowLike = globalThis as unknown as WindowLike
  ) {
    this.bootstrap = bootstrap;
    this.windowLike = windowLike;
  }

  invoke<TAction extends IdeInvokeAction>(
    action: TAction,
    args: IdeInvokeArgsByAction[TAction],
    options?: InvokeOptions
  ): Promise<IdeInvokeResultByAction[TAction]> {
    const timeoutMs = options?.timeoutMs ?? DEFAULT_INVOKE_TIMEOUT_MS;

    switch (this.bootstrap.host) {
      case "vscode":
        return this.invokeViaVsCode(action, args, timeoutMs);
      case "intellij":
        return this.invokeViaIntelliJ(action, args, timeoutMs);
      default:
        return Promise.reject(
          toHostBridgeError(
            "UNSUPPORTED_HOST",
            `IDE host bridge is not supported for host ${this.bootstrap.host}`
          )
        );
    }
  }

  async getCapabilities(options?: InvokeOptions): Promise<IdeCapabilities> {
    const result = await this.invoke("ide.getCapabilities", {}, options);
    return result.capabilities;
  }

  dispose(): void {
    if (this.isVsCodeMessageListenerAttached) {
      this.windowLike.removeEventListener?.("message", this.onVsCodeMessage);
      this.isVsCodeMessageListenerAttached = false;
    }

    for (const pending of this.pendingInvokes.values()) {
      clearTimeout(pending.timeoutHandle);
      pending.reject(
        toHostBridgeError("OPEN_FAILED", "Host bridge client disposed before invoke resolved")
      );
    }

    this.pendingInvokes.clear();
  }

  private invokeViaVsCode<TAction extends IdeInvokeAction>(
    action: TAction,
    args: IdeInvokeArgsByAction[TAction],
    timeoutMs: number
  ): Promise<IdeInvokeResultByAction[TAction]> {
    const api = this.getVsCodeApi();
    if (!api || typeof api.postMessage !== "function") {
      return Promise.reject(
        toHostBridgeError("UNSUPPORTED_HOST", "VSCode webview API is unavailable")
      );
    }

    this.ensureVsCodeListener();

    const message = createHostBridgeInvokeMessage(action, args);

    return new Promise<IdeInvokeResultByAction[TAction]>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        this.pendingInvokes.delete(message.id);
        reject(
          toHostBridgeError("TIMEOUT", `Host bridge invoke timed out for action ${action} (${timeoutMs}ms)`)
        );
      }, timeoutMs);

      this.pendingInvokes.set(message.id, {
        action,
        resolve: (result) => resolve(result as IdeInvokeResultByAction[TAction]),
        reject,
        timeoutHandle
      });

      let postResult: unknown;
      try {
        postResult = api.postMessage(message);
      } catch (error) {
        clearTimeout(timeoutHandle);
        this.pendingInvokes.delete(message.id);
        reject(toHostBridgeError("OPEN_FAILED", error instanceof Error ? error.message : String(error)));
        return;
      }

      // In VSCode webviews, postMessage often returns void. Treat only an explicit false as rejection.
      if (postResult === false) {
        clearTimeout(timeoutHandle);
        this.pendingInvokes.delete(message.id);
        reject(
          toHostBridgeError("OPEN_FAILED", `VSCode host rejected bridge message for action ${action}`)
        );
      }
    });
  }

  private getVsCodeApi(): VsCodeWebviewApi | null {
    if (this.vsCodeApi !== undefined) {
      return this.vsCodeApi;
    }

    try {
      const api = this.windowLike.acquireVsCodeApi?.();
      this.vsCodeApi = api && typeof api.postMessage === "function" ? api : null;
    } catch {
      this.vsCodeApi = null;
    }

    return this.vsCodeApi;
  }

  private invokeViaIntelliJ<TAction extends IdeInvokeAction>(
    action: TAction,
    args: IdeInvokeArgsByAction[TAction],
    timeoutMs: number
  ): Promise<IdeInvokeResultByAction[TAction]> {
    const invoke = this.windowLike.__HARMONY_INTELLIJ_HOST_INVOKE__;
    if (typeof invoke !== "function") {
      return Promise.reject(
        toHostBridgeError("UNSUPPORTED_HOST", "IntelliJ host bridge API is unavailable")
      );
    }

    const request = createHostBridgeInvokeMessage(action, args);

    return new Promise<IdeInvokeResultByAction[TAction]>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        reject(
          toHostBridgeError("TIMEOUT", `Host bridge invoke timed out for action ${action} (${timeoutMs}ms)`)
        );
      }, timeoutMs);

      Promise.resolve(invoke(request))
        .then((rawResponse) => {
          clearTimeout(timeoutHandle);

          if (!isHostBridgeResultOrError(rawResponse)) {
            reject(
              toHostBridgeError(
                "OPEN_FAILED",
                `IntelliJ host returned invalid bridge response for action ${action}`
              )
            );
            return;
          }

          if (rawResponse.id !== request.id) {
            reject(
              toHostBridgeError(
                "OPEN_FAILED",
                `IntelliJ host returned mismatched invoke id for action ${action}`
              )
            );
            return;
          }

          if (rawResponse.payload.action !== action) {
            reject(
              toHostBridgeError(
                "OPEN_FAILED",
                `Unexpected IntelliJ host action: expected ${action}, got ${rawResponse.payload.action}`
              )
            );
            return;
          }

          if (rawResponse.type === "error") {
            reject(
              toHostBridgeError(
                normalizeHostBridgeErrorCode(rawResponse.payload.code),
                rawResponse.payload.message
              )
            );
            return;
          }

          resolve(rawResponse.payload.data as IdeInvokeResultByAction[TAction]);
        })
        .catch((error: unknown) => {
          clearTimeout(timeoutHandle);
          reject(toHostBridgeError("OPEN_FAILED", error instanceof Error ? error.message : String(error)));
        });
    });
  }

  private ensureVsCodeListener(): void {
    if (this.isVsCodeMessageListenerAttached) {
      return;
    }

    this.windowLike.addEventListener?.("message", this.onVsCodeMessage);
    this.isVsCodeMessageListenerAttached = true;
  }
}

export function createHostBridgeClient(
  windowLike: WindowLike = globalThis as unknown as WindowLike
): HarmonyHostBridgeClient {
  return new HarmonyHostBridgeClient(resolveBootstrap(windowLike), windowLike);
}

export class HarmonyWebSocketClient {
  private readonly bootstrap: BridgeBootstrap;
  private socket?: WebSocket;
  private state: ConnectionState = "idle";
  private readonly statusListeners = new Set<StatusListener>();
  private readonly messageListeners = new Set<MessageListener>();
  private readonly pendingInvokes = new Map<string, PendingInvoke>();

  constructor(bootstrap: BridgeBootstrap) {
    this.bootstrap = bootstrap;
  }

  connect(): void {
    if (this.socket && this.socket.readyState < WebSocket.CLOSING) {
      return;
    }

    this.setState("connecting");
    this.socket = new WebSocket(this.bootstrap.wsUrl);

    this.socket.addEventListener("open", () => {
      this.setState("open");
    });

    this.socket.addEventListener("close", () => {
      this.setState("closed");
      this.rejectPendingInvokes("WebSocket closed before invoke resolved");
    });

    this.socket.addEventListener("error", () => {
      this.setState("error");
    });

    this.socket.addEventListener("message", (event) => {
      try {
        const parsed = JSON.parse(event.data as string) as HostMessage;
        this.handleHostMessage(parsed);
        this.messageListeners.forEach((listener) => listener(parsed));
      } catch {
        const decodeError: HostMessage = {
          id: "decode-error",
          type: "error",
          payload: {
            code: "DECODE_ERROR",
            message: "Received non-protocol websocket payload"
          },
          ts: Date.now()
        };
        this.messageListeners.forEach((listener) => listener(decodeError));
      }
    });
  }

  send(message: ClientMessage): void {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return;
    }

    this.socket.send(JSON.stringify(message));
  }

  invoke<TAction extends InvokeAction>(
    action: TAction,
    args: InvokeArgsByAction[TAction],
    options?: InvokeOptions
  ): Promise<InvokeResultByAction[TAction]> {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error("WebSocket is not open"));
    }

    const timeoutMs = options?.timeoutMs ?? DEFAULT_INVOKE_TIMEOUT_MS;
    const message = createEnvelope("invoke", { action, args }) as ClientMessage;

    return new Promise<InvokeResultByAction[TAction]>((resolve, reject) => {
      const timeoutHandle = setTimeout(() => {
        this.pendingInvokes.delete(message.id);
        reject(new Error(`Invoke timed out for action ${action} (${timeoutMs}ms)`));
      }, timeoutMs);

      this.pendingInvokes.set(message.id, {
        action,
        resolve: (result) => resolve(result as InvokeResultByAction[TAction]),
        reject,
        timeoutHandle
      });

      try {
        this.socket?.send(JSON.stringify(message));
      } catch (error) {
        clearTimeout(timeoutHandle);
        this.pendingInvokes.delete(message.id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  onStatus(listener: StatusListener): () => void {
    this.statusListeners.add(listener);
    listener(this.state);
    return () => {
      this.statusListeners.delete(listener);
    };
  }

  onMessage(listener: MessageListener): () => void {
    this.messageListeners.add(listener);
    return () => {
      this.messageListeners.delete(listener);
    };
  }

  dispose(): void {
    this.socket?.close();
    this.socket = undefined;
    this.setState("closed");
    this.rejectPendingInvokes("WebSocket client disposed before invoke resolved");
  }

  private setState(next: ConnectionState): void {
    this.state = next;
    this.statusListeners.forEach((listener) => listener(next));
  }

  private handleHostMessage(message: HostMessage): void {
    const pending = this.pendingInvokes.get(message.id);
    if (!pending) {
      return;
    }

    clearTimeout(pending.timeoutHandle);
    this.pendingInvokes.delete(message.id);

    if (message.type === "error") {
      pending.reject(new Error(`[${message.payload.code}] ${message.payload.message}`));
      return;
    }

    const expectedEventName = actionResultEventName(pending.action);
    if (message.payload.name !== expectedEventName) {
      pending.reject(
        new Error(
          `Unexpected invoke result for ${pending.action}: expected ${expectedEventName}, got ${message.payload.name}`
        )
      );
      return;
    }

    pending.resolve(message.payload.data ?? {});
  }

  private rejectPendingInvokes(reason: string): void {
    for (const pending of this.pendingInvokes.values()) {
      clearTimeout(pending.timeoutHandle);
      pending.reject(new Error(reason));
    }

    this.pendingInvokes.clear();
  }
}
