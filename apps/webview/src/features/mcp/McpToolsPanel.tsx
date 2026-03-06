import type { McpToolSummary } from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { useEffect, useState } from "react";

interface McpToolsPanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
}

type LoadStatus = "idle" | "loading" | "ready" | "error";

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
      <p className="kicker">MCP</p>
      <h2>MCP tools</h2>
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
  const [status, setStatus] = useState<LoadStatus>("idle");
  const [tools, setTools] = useState<McpToolSummary[]>([]);
  const [errorMessage, setErrorMessage] = useState<string>();

  useEffect(() => {
    let cancelled = false;

    if (!client || connectionState !== "open") {
      setStatus("idle");
      setTools([]);
      setErrorMessage(undefined);
      return;
    }

    const loadTools = async () => {
      setStatus("loading");
      setErrorMessage(undefined);

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
        setErrorMessage(toErrorMessage(error));
      }
    };

    void loadTools();

    return () => {
      cancelled = true;
    };
  }, [client, connectionState]);

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
        errorMessage={errorMessage}
      />
    );
  }

  return (
    <section className="panel mcp-tools-panel" aria-label="MCP tools">
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
