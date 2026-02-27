import * as vscode from "vscode";
import { WebSocketServer, type RawData, type WebSocket } from "ws";

const BRIDGE_PORT = 8788;
let bridgeServer: WebSocketServer | undefined;

function startBridgeServer(output: vscode.OutputChannel): void {
  if (bridgeServer) {
    return;
  }

  bridgeServer = new WebSocketServer({ host: "127.0.0.1", port: BRIDGE_PORT });

  bridgeServer.on("connection", (socket: WebSocket) => {
    socket.on("message", (raw: RawData) => {
      const payloadText = raw.toString();

      try {
        const incoming = JSON.parse(payloadText) as {
          id?: string;
          type?: string;
        };

        const outgoing =
          incoming.type === "ping"
            ? {
                id: incoming.id ?? "vscode-pong",
                type: "pong",
                payload: { host: "vscode", note: "pong from vscode bridge" },
                ts: Date.now()
              }
            : {
                id: incoming.id ?? "vscode-event",
                type: "event",
                payload: {
                  name: "invoke.received",
                  data: {
                    host: "vscode",
                    receivedType: incoming.type ?? "unknown"
                  }
                },
                ts: Date.now()
              };

        socket.send(JSON.stringify(outgoing));
      } catch {
        socket.send(
          JSON.stringify({
            id: "decode-error",
            type: "error",
            payload: {
              code: "INVALID_MESSAGE",
              message: "Expected Harmony protocol JSON envelope"
            },
            ts: Date.now()
          })
        );
      }
    });
  });

  bridgeServer.on("listening", () => {
    output.appendLine(`Harmony websocket bridge listening on ws://127.0.0.1:${BRIDGE_PORT}`);
  });

  bridgeServer.on("error", (error: Error) => {
    output.appendLine(`Harmony websocket bridge error: ${error.message}`);
  });
}

function buildWebviewHtml(panel: vscode.WebviewPanel, extensionUri: vscode.Uri): string {
  const distRoot = vscode.Uri.joinPath(extensionUri, "media", "webview");
  const scriptUri = panel.webview.asWebviewUri(vscode.Uri.joinPath(distRoot, "assets", "main.js"));
  const styleUri = panel.webview.asWebviewUri(vscode.Uri.joinPath(distRoot, "assets", "main.css"));
  const nonce = `${Date.now()}`;

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${panel.webview.cspSource} data:; style-src ${panel.webview.cspSource}; script-src 'nonce-${nonce}'; connect-src ws://127.0.0.1:${BRIDGE_PORT};" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="stylesheet" href="${styleUri}" />
    <title>Harmony</title>
    <script nonce="${nonce}">
      window.__HARMONY_BRIDGE__ = { host: "vscode", wsUrl: "ws://127.0.0.1:${BRIDGE_PORT}" };
    </script>
  </head>
  <body>
    <div id="root"></div>
    <script nonce="${nonce}" type="module" src="${scriptUri}"></script>
  </body>
</html>`;
}

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("Harmony");
  startBridgeServer(output);

  const openCommand = vscode.commands.registerCommand("harmony.openWebview", () => {
    const panel = vscode.window.createWebviewPanel("harmony", "Harmony", vscode.ViewColumn.One, {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")]
    });

    panel.webview.html = buildWebviewHtml(panel, context.extensionUri);
  });

  context.subscriptions.push(openCommand, output, {
    dispose: () => {
      bridgeServer?.close();
      bridgeServer = undefined;
    }
  });
}

export function deactivate(): void {
  bridgeServer?.close();
  bridgeServer = undefined;
}
