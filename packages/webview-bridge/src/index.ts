import {
  actionResultEventName,
  createEnvelope,
  type BridgeBootstrap,
  type ClientMessage,
  type HarmonyHost,
  type HostMessage,
  type InvokeAction,
  type InvokeArgsByAction,
  type InvokeResultByAction
} from "@harmony/protocol";

export type ConnectionState = "idle" | "connecting" | "open" | "closed" | "error";

type StatusListener = (state: ConnectionState) => void;
type MessageListener = (message: HostMessage) => void;

interface PendingInvoke {
  action: InvokeAction;
  resolve: (result: unknown) => void;
  reject: (error: Error) => void;
  timeoutHandle: ReturnType<typeof setTimeout>;
}

interface LocationLike {
  search: string;
}

interface WindowLike {
  __HARMONY_BRIDGE__?: BridgeBootstrap;
  __TAURI__?: unknown;
  acquireVsCodeApi?: () => unknown;
  location?: LocationLike;
}

export interface InvokeOptions {
  timeoutMs?: number;
}

const DEFAULT_INVOKE_TIMEOUT_MS = 8_000;

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
