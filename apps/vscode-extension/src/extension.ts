import { spawn, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as net from "node:net";
import * as path from "node:path";
import type {
  HostBridgeErrorMessage,
  HostBridgeInvokeMessage,
  HostBridgeResultMessage
} from "@harmony/protocol";
import * as vscode from "vscode";

const BRIDGE_HOST = "127.0.0.1";
const BRIDGE_PORT = 8788;
const BRIDGE_WS_URL = `ws://${BRIDGE_HOST}:${BRIDGE_PORT}`;
const HARMONY_VIEW_ID = "harmony-main-view";
const READY_TIMEOUT_MS = 8_000;
const READY_POLL_INTERVAL_MS = 150;
const HOST_BRIDGE_CHANNEL = "harmony-host";

let bridgeProcess: ChildProcess | undefined;
let bridgeStartup: Promise<void> | undefined;

type HostBridgeAction = "ide.getCapabilities" | "ide.openFile";
type HostBridgeErrorCode =
  | "UNSUPPORTED_HOST"
  | "INVALID_ARGS"
  | "FILE_NOT_FOUND"
  | "OPEN_FAILED"
  | "TIMEOUT";

interface IdeOpenFileArgs {
  path: string;
  line?: number;
  column?: number;
  preview?: boolean;
  preserveFocus?: boolean;
}

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

function findBundledBridgeBinary(extensionRoot: string): string | undefined {
  const candidates = [
    path.join(extensionRoot, "bin", "hdc-bridge-rs"),
    path.join(extensionRoot, "bin", "hdc-bridge-rs.exe")
  ];

  return candidates.find((candidate) => fs.existsSync(candidate));
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
  const bundledBinary = findBundledBridgeBinary(context.extensionUri.fsPath);
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

  if (bundledBinary) {
    output.appendLine(`[hdc-bridge] Launching bundled binary: ${bundledBinary}`);
    const child = spawn(bundledBinary, ["--ws-addr", wsAddr], {
      cwd: path.dirname(bundledBinary),
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

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isHostBridgeAction(value: unknown): value is HostBridgeAction {
  return value === "ide.getCapabilities" || value === "ide.openFile";
}

function isHostBridgeInvokeMessage(value: unknown): value is HostBridgeInvokeMessage {
  if (!isObjectRecord(value)) {
    return false;
  }

  if (value.channel !== HOST_BRIDGE_CHANNEL || value.type !== "invoke" || typeof value.id !== "string") {
    return false;
  }

  if (!isObjectRecord(value.payload)) {
    return false;
  }

  if (!isHostBridgeAction(value.payload.action)) {
    return false;
  }

  return isObjectRecord(value.payload.args);
}

function createHostBridgeCapabilitiesResult(id: string): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.getCapabilities",
      data: {
        capabilities: {
          "ide.openFile": true
        }
      }
    }
  };
}

function createHostBridgeOpenFileResult(id: string): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.openFile",
      data: {
        opened: true
      }
    }
  };
}

function createHostBridgeError(
  id: string,
  action: HostBridgeAction,
  code: HostBridgeErrorCode,
  message: string
): HostBridgeErrorMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "error",
    payload: {
      action,
      code,
      message
    }
  };
}

function parseOpenFileArgs(args: unknown): IdeOpenFileArgs | string {
  if (!isObjectRecord(args)) {
    return "`args` must be an object";
  }

  const filePath = args.path;
  if (typeof filePath !== "string" || filePath.trim().length === 0) {
    return "`path` must be a non-empty string";
  }

  if (!path.isAbsolute(filePath)) {
    return "`path` must be an absolute filesystem path";
  }

  const line = args.line;
  if (line !== undefined && (typeof line !== "number" || !Number.isInteger(line) || line <= 0)) {
    return "`line` must be a positive integer when provided";
  }

  const column = args.column;
  if (column !== undefined && (typeof column !== "number" || !Number.isInteger(column) || column <= 0)) {
    return "`column` must be a positive integer when provided";
  }

  const preview = args.preview;
  if (preview !== undefined && typeof preview !== "boolean") {
    return "`preview` must be a boolean when provided";
  }

  const preserveFocus = args.preserveFocus;
  if (preserveFocus !== undefined && typeof preserveFocus !== "boolean") {
    return "`preserveFocus` must be a boolean when provided";
  }

  return {
    path: filePath,
    line: line as number | undefined,
    column: column as number | undefined,
    preview,
    preserveFocus
  };
}

async function openFileInEditor(args: IdeOpenFileArgs): Promise<void> {
  if (!fs.existsSync(args.path)) {
    throw new Error("FILE_NOT_FOUND");
  }

  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(args.path));
  const requestedLine = (args.line ?? 1) - 1;
  const line = Math.max(0, Math.min(requestedLine, Math.max(0, document.lineCount - 1)));
  const maxColumn = document.lineAt(line).text.length;
  const requestedColumn = (args.column ?? 1) - 1;
  const column = Math.max(0, Math.min(requestedColumn, maxColumn));
  const position = new vscode.Position(line, column);

  await vscode.window.showTextDocument(document, {
    preview: args.preview ?? false,
    preserveFocus: args.preserveFocus ?? false,
    selection: new vscode.Selection(position, position)
  });
}

async function handleHostBridgeInvoke(
  raw: unknown,
  webview: vscode.Webview,
  output: vscode.OutputChannel
): Promise<void> {
  if (!isHostBridgeInvokeMessage(raw)) {
    return;
  }

  const { id, payload } = raw;
  const { action } = payload;

  if (action === "ide.getCapabilities") {
    await webview.postMessage(createHostBridgeCapabilitiesResult(id));
    return;
  }

  const parsedArgs = parseOpenFileArgs(payload.args);
  if (typeof parsedArgs === "string") {
    await webview.postMessage(createHostBridgeError(id, action, "INVALID_ARGS", parsedArgs));
    return;
  }

  try {
    await openFileInEditor(parsedArgs);
    await webview.postMessage(createHostBridgeOpenFileResult(id));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const code: HostBridgeErrorCode = message === "FILE_NOT_FOUND" ? "FILE_NOT_FOUND" : "OPEN_FAILED";
    output.appendLine(`[host-bridge:${code}] ${message}`);
    await webview.postMessage(createHostBridgeError(id, action, code, message));
  }
}

function buildWebviewHtml(webview: vscode.Webview, extensionUri: vscode.Uri): string {
  const distRoot = vscode.Uri.joinPath(extensionUri, "media", "webview");
  const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(distRoot, "assets", "main.js"));
  const styleUri = webview.asWebviewUri(vscode.Uri.joinPath(distRoot, "assets", "main.css"));
  const nonce = `${Date.now()}`;

  return `<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource}; script-src 'nonce-${nonce}'; connect-src ${BRIDGE_WS_URL};" />
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
  const mediaRoot = vscode.Uri.joinPath(context.extensionUri, "media");

  const webviewProvider = vscode.window.registerWebviewViewProvider(
    HARMONY_VIEW_ID,
    {
      resolveWebviewView: async (webviewView) => {
        webviewView.webview.options = {
          enableScripts: true,
          localResourceRoots: [mediaRoot]
        };

        const onDidReceiveMessage = webviewView.webview.onDidReceiveMessage((raw) => {
          void handleHostBridgeInvoke(raw, webviewView.webview, output);
        });
        context.subscriptions.push(onDidReceiveMessage);

        try {
          await ensureBridgeStarted(context, output);
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          output.appendLine(`[hdc-bridge:error] ${message}`);
        }

        webviewView.webview.html = buildWebviewHtml(webviewView.webview, context.extensionUri);
      }
    },
    {
      webviewOptions: {
        retainContextWhenHidden: true
      }
    }
  );

  context.subscriptions.push(webviewProvider, output, {
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
