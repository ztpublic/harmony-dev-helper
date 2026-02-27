import { spawn, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as net from "node:net";
import * as path from "node:path";
import * as vscode from "vscode";

const BRIDGE_HOST = "127.0.0.1";
const BRIDGE_PORT = 8788;
const BRIDGE_WS_URL = `ws://${BRIDGE_HOST}:${BRIDGE_PORT}`;
const READY_TIMEOUT_MS = 8_000;
const READY_POLL_INTERVAL_MS = 150;

let bridgeProcess: ChildProcess | undefined;
let bridgeStartup: Promise<void> | undefined;

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function probeBridgePort(): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = net.createConnection({ host: BRIDGE_HOST, port: BRIDGE_PORT });

    const finish = (open: boolean) => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(open);
    };

    socket.once("connect", () => finish(true));
    socket.once("error", () => finish(false));
    socket.setTimeout(300, () => finish(false));
  });
}

async function waitForBridgePort(timeoutMs: number): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;

  while (Date.now() < deadline) {
    if (await probeBridgePort()) {
      return true;
    }

    await sleep(READY_POLL_INTERVAL_MS);
  }

  return probeBridgePort();
}

function findBridgeManifest(startDir: string): string | undefined {
  const fromEnv = process.env.HARMONY_HDC_BRIDGE_MANIFEST_PATH?.trim();
  if (fromEnv) {
    return fs.existsSync(fromEnv) ? fromEnv : undefined;
  }

  let current = startDir;
  while (true) {
    const candidate = path.join(current, "apps", "hdc-bridge-rs", "Cargo.toml");
    if (fs.existsSync(candidate)) {
      return candidate;
    }

    const parent = path.dirname(current);
    if (parent === current) {
      break;
    }

    current = parent;
  }

  return undefined;
}

function attachProcessLogging(processRef: ChildProcess, output: vscode.OutputChannel): void {
  processRef.stdout?.on("data", (chunk: Buffer) => {
    const message = chunk.toString().trim();
    if (message) {
      output.appendLine(`[hdc-bridge] ${message}`);
    }
  });

  processRef.stderr?.on("data", (chunk: Buffer) => {
    const message = chunk.toString().trim();
    if (message) {
      output.appendLine(`[hdc-bridge:error] ${message}`);
    }
  });

  processRef.on("error", (error: Error) => {
    output.appendLine(`[hdc-bridge:error] Failed to start sidecar: ${error.message}`);
  });

  processRef.on("exit", (code: number | null, signal: NodeJS.Signals | null) => {
    output.appendLine(`[hdc-bridge] sidecar exited (code=${code}, signal=${signal ?? "none"})`);
    if (bridgeProcess === processRef) {
      bridgeProcess = undefined;
    }
  });
}

function spawnBridgeProcess(context: vscode.ExtensionContext, output: vscode.OutputChannel): ChildProcess {
  const binaryOverride = process.env.HARMONY_HDC_BRIDGE_BIN?.trim();
  const wsAddr = `${BRIDGE_HOST}:${BRIDGE_PORT}`;

  if (binaryOverride) {
    output.appendLine(`[hdc-bridge] Launching binary override: ${binaryOverride}`);
    const child = spawn(binaryOverride, ["--ws-addr", wsAddr], {
      cwd: path.dirname(binaryOverride),
      stdio: ["ignore", "pipe", "pipe"]
    });
    attachProcessLogging(child, output);
    return child;
  }

  const manifestPath = findBridgeManifest(context.extensionUri.fsPath);
  if (!manifestPath) {
    throw new Error(
      "Could not locate apps/hdc-bridge-rs/Cargo.toml. Set HARMONY_HDC_BRIDGE_BIN or HARMONY_HDC_BRIDGE_MANIFEST_PATH."
    );
  }

  output.appendLine(`[hdc-bridge] Launching via cargo run with manifest ${manifestPath}`);
  const child = spawn(
    "cargo",
    ["run", "--manifest-path", manifestPath, "--", "--ws-addr", wsAddr],
    {
      cwd: path.dirname(manifestPath),
      stdio: ["ignore", "pipe", "pipe"]
    }
  );
  attachProcessLogging(child, output);
  return child;
}

async function ensureBridgeStarted(context: vscode.ExtensionContext, output: vscode.OutputChannel): Promise<void> {
  if (await probeBridgePort()) {
    return;
  }

  if (bridgeStartup) {
    return bridgeStartup;
  }

  bridgeStartup = (async () => {
    if (!bridgeProcess || bridgeProcess.exitCode !== null) {
      bridgeProcess = spawnBridgeProcess(context, output);
    }

    const ready = await waitForBridgePort(READY_TIMEOUT_MS);
    if (!ready) {
      throw new Error(
        `Timed out waiting for HDC bridge sidecar on ws://${BRIDGE_HOST}:${BRIDGE_PORT}`
      );
    }

    output.appendLine(`[hdc-bridge] ready on ws://${BRIDGE_HOST}:${BRIDGE_PORT}`);
  })();

  try {
    await bridgeStartup;
  } finally {
    bridgeStartup = undefined;
  }
}

function stopBridgeProcess(output: vscode.OutputChannel): void {
  if (!bridgeProcess || bridgeProcess.exitCode !== null) {
    bridgeProcess = undefined;
    return;
  }

  output.appendLine("[hdc-bridge] stopping sidecar");
  bridgeProcess.kill();
  bridgeProcess = undefined;
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
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${panel.webview.cspSource} data:; style-src ${panel.webview.cspSource}; script-src 'nonce-${nonce}'; connect-src ${BRIDGE_WS_URL};" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <link rel="stylesheet" href="${styleUri}" />
    <title>Harmony</title>
    <script nonce="${nonce}">
      window.__HARMONY_BRIDGE__ = { host: "vscode", wsUrl: "${BRIDGE_WS_URL}" };
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

  void ensureBridgeStarted(context, output).catch((error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    output.appendLine(`[hdc-bridge:error] ${message}`);
  });

  const openCommand = vscode.commands.registerCommand("harmony.openWebview", async () => {
    try {
      await ensureBridgeStarted(context, output);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      output.appendLine(`[hdc-bridge:error] ${message}`);
    }

    const panel = vscode.window.createWebviewPanel("harmony", "Harmony", vscode.ViewColumn.One, {
      enableScripts: true,
      localResourceRoots: [vscode.Uri.joinPath(context.extensionUri, "media")]
    });

    panel.webview.html = buildWebviewHtml(panel, context.extensionUri);
  });

  context.subscriptions.push(openCommand, output, {
    dispose: () => {
      stopBridgeProcess(output);
    }
  });
}

export function deactivate(): void {
  if (bridgeProcess && bridgeProcess.exitCode === null) {
    bridgeProcess.kill();
  }

  bridgeProcess = undefined;
}
