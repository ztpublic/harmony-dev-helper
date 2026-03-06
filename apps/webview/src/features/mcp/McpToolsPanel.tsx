import type { McpToolSummary } from "@harmony/protocol";
import {
  createHostBridgeClient,
  type ConnectionState,
  type HarmonyHostBridgeClient,
  type HarmonyWebSocketClient
} from "@harmony/webview-bridge";
import { useEffect, useRef, useState } from "react";

interface McpToolsPanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
}

type LoadStatus = "idle" | "loading" | "ready" | "error";
type CursorAction = "add" | "remove" | null;

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function McpToolsPlaceholder({
  message,
  errorMessage
}: {
  message: string;
  errorMessage?: string;
}) {
  return (
    <section className="panel mcp-tools-panel" aria-label="MCP tools">
      <p className="panel-message">{message}</p>
      {errorMessage ? <p className="panel-message panel-message-error">{errorMessage}</p> : null}
    </section>
  );
}

function McpToolRow({ tool }: { tool: McpToolSummary }) {
  const title = tool.title?.trim();
  const description = tool.description?.trim() || "No description available.";

  return (
    <tr>
      <td>
        <div className="mcp-tool-cell">
          <code className="mcp-tool-name">{tool.name}</code>
          {title ? <span className="mcp-tool-title">{title}</span> : null}
        </div>
      </td>
      <td className="mcp-tool-description">{description}</td>
    </tr>
  );
}

export function McpToolsPanel({ client, connectionState }: McpToolsPanelProps) {
  const hostBridgeRef = useRef<HarmonyHostBridgeClient | null>(null);
  if (!hostBridgeRef.current) {
    hostBridgeRef.current = createHostBridgeClient();
  }

  const [status, setStatus] = useState<LoadStatus>("idle");
  const [tools, setTools] = useState<McpToolSummary[]>([]);
  const [toolErrorMessage, setToolErrorMessage] = useState<string>();
  const [showCursorToolbar, setShowCursorToolbar] = useState(false);
  const [cursorErrorMessage, setCursorErrorMessage] = useState<string>();
  const [cursorActionPending, setCursorActionPending] = useState<CursorAction>(null);

  useEffect(() => {
    const hostBridge = hostBridgeRef.current;

    return () => {
      hostBridge?.dispose();
      if (hostBridgeRef.current === hostBridge) {
        hostBridgeRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    if (!client || connectionState !== "open") {
      setStatus("idle");
      setTools([]);
      setToolErrorMessage(undefined);
      return;
    }

    const loadTools = async () => {
      setStatus("loading");
      setToolErrorMessage(undefined);

      try {
        const result = await client.invoke("mcp.listTools", {});
        if (cancelled) {
          return;
        }

        setTools(result.tools);
        setStatus("ready");
      } catch (error) {
        if (cancelled) {
          return;
        }

        setTools([]);
        setStatus("error");
        setToolErrorMessage(toErrorMessage(error));
      }
    };

    void loadTools();

    return () => {
      cancelled = true;
    };
  }, [client, connectionState]);

  useEffect(() => {
    let cancelled = false;
    const hostBridge = hostBridgeRef.current;

    if (!hostBridge) {
      setShowCursorToolbar(false);
      setCursorErrorMessage(undefined);
      return;
    }

    const detectCursorSupport = async () => {
      setCursorErrorMessage(undefined);

      let capabilities: Awaited<ReturnType<HarmonyHostBridgeClient["getCapabilities"]>>;
      let hostInfo: Awaited<ReturnType<HarmonyHostBridgeClient["getHostInfo"]>>;

      try {
        [capabilities, hostInfo] = await Promise.all([
          hostBridge.getCapabilities(),
          hostBridge.getHostInfo()
        ]);
      } catch {
        if (cancelled) {
          return;
        }

        setShowCursorToolbar(false);
        return;
      }

      if (cancelled) {
        return;
      }

      const cursorMcpSupported =
        hostInfo.host === "cursor" &&
        capabilities["ide.cursorMcp.addServer"] &&
        capabilities["ide.cursorMcp.removeServer"];

      setShowCursorToolbar(cursorMcpSupported);
    };

    void detectCursorSupport();

    return () => {
      cancelled = true;
    };
  }, []);

  const invokeCursorMcpAction = async (
    action: "ide.cursorMcp.addServer" | "ide.cursorMcp.removeServer",
    pendingState: Exclude<CursorAction, null>
  ) => {
    const hostBridge = hostBridgeRef.current;
    if (!hostBridge || !showCursorToolbar) {
      return;
    }

    setCursorActionPending(pendingState);
    setCursorErrorMessage(undefined);

    try {
      await hostBridge.invoke(action, {});
    } catch (error) {
      const actionLabel = pendingState === "add" ? "add Harmony to Cursor" : "remove Harmony from Cursor";
      setCursorErrorMessage(`Failed to ${actionLabel}. ${toErrorMessage(error)}`);
    } finally {
      setCursorActionPending(null);
    }
  };

  if (connectionState !== "open") {
    return <McpToolsPlaceholder message="Waiting for websocket connection." />;
  }

  if (status === "loading" || status === "idle") {
    return <McpToolsPlaceholder message="Loading MCP tools..." />;
  }

  if (status === "error") {
    return (
      <McpToolsPlaceholder
        message="Failed to load built-in MCP tools."
        errorMessage={toolErrorMessage}
      />
    );
  }

  return (
    <section className="panel mcp-tools-panel" aria-label="MCP tools">
      {showCursorToolbar ? (
        <div className="mcp-tools-toolbar">
          <button
            type="button"
            className="mcp-tools-toolbar-button-primary"
            disabled={cursorActionPending !== null}
            onClick={() => {
              void invokeCursorMcpAction("ide.cursorMcp.addServer", "add");
            }}
          >
            {cursorActionPending === "add" ? "Adding..." : "Add"}
          </button>
          <button
            type="button"
            className="mcp-tools-toolbar-button-secondary"
            disabled={cursorActionPending !== null}
            onClick={() => {
              void invokeCursorMcpAction("ide.cursorMcp.removeServer", "remove");
            }}
          >
            {cursorActionPending === "remove" ? "Removing..." : "Remove"}
          </button>
        </div>
      ) : null}
      {cursorErrorMessage ? (
        <p className="panel-message panel-message-error">{cursorErrorMessage}</p>
      ) : null}
      {tools.length === 0 ? (
        <p className="panel-message">No MCP tools are currently registered.</p>
      ) : (
        <div className="mcp-tools-table-shell">
          <table className="mcp-tools-table">
            <thead>
              <tr>
                <th scope="col">Tool</th>
                <th scope="col">Description</th>
              </tr>
            </thead>
            <tbody>
              {tools.map((tool) => (
                <McpToolRow key={tool.name} tool={tool} />
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
