import type { BridgeBootstrap, ClientMessage, HarmonyHost, HostMessage } from "@harmony/protocol";

export type ConnectionState = "idle" | "connecting" | "open" | "closed" | "error";

type StatusListener = (state: ConnectionState) => void;
type MessageListener = (message: HostMessage) => void;

interface LocationLike {
  search: string;
}

interface WindowLike {
  __HARMONY_BRIDGE__?: BridgeBootstrap;
  __TAURI__?: unknown;
  acquireVsCodeApi?: () => unknown;
  location?: LocationLike;
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
    });

    this.socket.addEventListener("error", () => {
      this.setState("error");
    });

    this.socket.addEventListener("message", (event) => {
      try {
        const parsed = JSON.parse(event.data as string) as HostMessage;
        this.messageListeners.forEach((listener) => listener(parsed));
      } catch {
        this.messageListeners.forEach((listener) =>
          listener({
            id: "decode-error",
            type: "error",
            payload: {
              code: "DECODE_ERROR",
              message: "Received non-protocol websocket payload"
            },
            ts: Date.now()
          })
        );
      }
    });
  }

  send(message: ClientMessage): void {
    if (!this.socket || this.socket.readyState !== WebSocket.OPEN) {
      return;
    }

    this.socket.send(JSON.stringify(message));
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
  }

  private setState(next: ConnectionState): void {
    this.state = next;
    this.statusListeners.forEach((listener) => listener(next));
  }
}
