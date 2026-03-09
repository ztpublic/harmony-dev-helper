import { spawn, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as net from "node:net";
import * as os from "node:os";
import * as path from "node:path";
import type {
  HostBridgeErrorMessage,
  HostBridgeInvokeMessage,
  HostBridgeResultMessage,
  IdeHostInfo
} from "@harmony/protocol";
import * as vscode from "vscode";

declare module "vscode" {
  export namespace cursor {
    export namespace mcp {
      export interface StdioServerConfig {
        name: string;
        server: {
          command: string;
          args: string[];
          env: Record<string, string>;
        };
      }

      export interface RemoteServerConfig {
        name: string;
        server: {
          url: string;
          headers?: Record<string, string>;
        };
      }

      export type ExtMCPServerConfig = StdioServerConfig | RemoteServerConfig;

      export const registerServer: (config: ExtMCPServerConfig) => void;
      export const unregisterServer: (serverName: string) => void;
    }
  }
}

const BRIDGE_HOST = "127.0.0.1";
const BRIDGE_PORT = 8788;
const BRIDGE_WS_URL = `ws://${BRIDGE_HOST}:${BRIDGE_PORT}`;
const HARMONY_MCP_PORT_OFFSET = 100;
const HARMONY_CURSOR_MCP_SERVER_NAME = "harmony-dev-helper";
const HARMONY_PANEL_ID = "harmony-panel";
const HARMONY_VIEW_ID = "harmony-main-view";
const OPEN_HARMONY_HELPER_VIEW_COMMAND_ID = "harmony.openHelperView";
const HARMONY_VIEW_OPENED_STATE_KEY = "harmony.hasOpenedHelperView";
const READY_TIMEOUT_MS = 8_000;
const READY_POLL_INTERVAL_MS = 150;
const HOST_BRIDGE_CHANNEL = "harmony-host";

let bridgeProcess: ChildProcess | undefined;
let bridgeStartup: Promise<void> | undefined;
let extensionOutputChannel: vscode.OutputChannel | undefined;
const trackedHarmonyTempFilePaths = new Set<string>();

type HostBridgeAction =
  | "ide.getCapabilities"
  | "ide.getHostInfo"
  | "ide.openFile"
  | "ide.openPath"
  | "ide.openExternal"
  | "ide.openChat"
  | "ide.openFilePicker"
  | "ide.cursorMcp.addServer"
  | "ide.cursorMcp.removeServer";
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

interface IdeOpenPathArgs {
  path: string;
  line?: number;
  column?: number;
  preview?: boolean;
  preserveFocus?: boolean;
}

interface IdeOpenExternalArgs {
  url: string;
}

interface IdeOpenChatArgs {
  query: string;
  isPartialQuery?: boolean;
}

interface IdeOpenFilePickerArgs {
  canSelectFiles?: boolean;
  canSelectFolders?: boolean;
  canSelectMany?: boolean;
  title?: string;
  defaultPath?: string;
  filters?: Record<string, string[]>;
}

interface IdeOpenFilePickerResult {
  canceled: boolean;
  paths: string[];
}

interface IdeCursorMcpStatusResult {
  added: boolean;
}

interface CursorMcpApiNamespace {
  registerServer: (config: vscode.cursor.mcp.ExtMCPServerConfig) => void;
  unregisterServer: (serverName: string) => void;
}

function harmonyOpenInEditorTempRootPath(): string {
  return path.resolve(os.tmpdir(), "harmony-dev-helper", "open-in-editor");
}

function isTrackedHarmonyOpenInEditorTempPath(filePath: string): boolean {
  const resolvedPath = path.resolve(filePath);
  const rootPath = harmonyOpenInEditorTempRootPath();
  return resolvedPath === rootPath || resolvedPath.startsWith(`${rootPath}${path.sep}`);
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

function harmonyMcpServerUrl(): string {
  return `http://${BRIDGE_HOST}:${BRIDGE_PORT + HARMONY_MCP_PORT_OFFSET}/mcp`;
}

function getCursorMcpApi(): CursorMcpApiNamespace | undefined {
  const cursorApi = (
    vscode as typeof vscode & {
      cursor?: {
        mcp?: Partial<CursorMcpApiNamespace>;
      };
    }
  ).cursor?.mcp;

  if (
    !cursorApi ||
    typeof cursorApi.registerServer !== "function" ||
    typeof cursorApi.unregisterServer !== "function"
  ) {
    return undefined;
  }

  return cursorApi as CursorMcpApiNamespace;
}

function isCursorMcpSupported(hostInfo: IdeHostInfo): boolean {
  return hostInfo.host === "cursor" && Boolean(getCursorMcpApi());
}

function isObjectRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isHostBridgeAction(value: unknown): value is HostBridgeAction {
  return (
    value === "ide.getCapabilities" ||
    value === "ide.getHostInfo" ||
    value === "ide.openFile" ||
    value === "ide.openPath" ||
    value === "ide.openExternal" ||
    value === "ide.openChat" ||
    value === "ide.openFilePicker" ||
    value === "ide.cursorMcp.addServer" ||
    value === "ide.cursorMcp.removeServer"
  );
}

function detectHostIdeInfo(): IdeHostInfo {
  const uriScheme = vscode.env.uriScheme ?? "";
  const appName = vscode.env.appName ?? "";
  const normalizedScheme = uriScheme.trim().toLowerCase();
  const normalizedAppName = appName.trim().toLowerCase();

  if (normalizedScheme === "cursor") {
    return {
      host: "cursor",
      uriScheme,
      appName,
      isOfficialVsCode: false
    };
  }

  if (normalizedScheme === "trae") {
    return {
      host: "trae",
      uriScheme,
      appName,
      isOfficialVsCode: false
    };
  }

  if (normalizedScheme === "vscode" || normalizedScheme === "vscode-insiders") {
    return {
      host: "vscode",
      uriScheme,
      appName,
      isOfficialVsCode: normalizedAppName.includes("visual studio code")
    };
  }

  if (normalizedAppName.includes("cursor")) {
    return {
      host: "cursor",
      uriScheme,
      appName,
      isOfficialVsCode: false
    };
  }

  if (normalizedAppName.includes("trae")) {
    return {
      host: "trae",
      uriScheme,
      appName,
      isOfficialVsCode: false
    };
  }

  if (normalizedAppName.includes("visual studio code")) {
    return {
      host: "vscode",
      uriScheme,
      appName,
      isOfficialVsCode: true
    };
  }

  return {
    host: "unknown",
    uriScheme,
    appName,
    isOfficialVsCode: false
  };
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

function createHostBridgeCapabilitiesResult(
  id: string,
  hostInfo: IdeHostInfo
): HostBridgeResultMessage {
  const cursorMcpSupported = isCursorMcpSupported(hostInfo);

  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.getCapabilities",
      data: {
        capabilities: {
          "ide.openFile": true,
          "ide.openPath": true,
          "ide.openExternal": true,
          "ide.openChat": true,
          "ide.openFilePicker": true,
          "ide.cursorMcp.addServer": cursorMcpSupported,
          "ide.cursorMcp.removeServer": cursorMcpSupported
        }
      }
    }
  };
}

function createHostBridgeHostInfoResult(id: string, host: IdeHostInfo): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.getHostInfo",
      data: {
        host
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

function createHostBridgeOpenPathResult(id: string, opened: boolean): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.openPath",
      data: {
        opened
      }
    }
  };
}

function createHostBridgeOpenExternalResult(id: string, opened: boolean): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.openExternal",
      data: {
        opened
      }
    }
  };
}

function createHostBridgeOpenChatResult(id: string, opened: boolean): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.openChat",
      data: {
        opened
      }
    }
  };
}

function createHostBridgeOpenFilePickerResult(
  id: string,
  result: IdeOpenFilePickerResult
): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action: "ide.openFilePicker",
      data: result
    }
  };
}

function createHostBridgeCursorMcpStatusResult(
  id: string,
  action: "ide.cursorMcp.addServer" | "ide.cursorMcp.removeServer",
  result: IdeCursorMcpStatusResult
): HostBridgeResultMessage {
  return {
    channel: HOST_BRIDGE_CHANNEL,
    id,
    type: "result",
    payload: {
      action,
      data: result
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

function parsePositiveInteger(value: unknown, fieldName: string): number | string | undefined {
  if (value === undefined) {
    return undefined;
  }

  if (typeof value !== "number" || !Number.isInteger(value) || value <= 0) {
    return `\`${fieldName}\` must be a positive integer when provided`;
  }

  return value;
}

function parseOptionalBoolean(value: unknown, fieldName: string): boolean | string | undefined {
  if (value === undefined) {
    return undefined;
  }

  if (typeof value !== "boolean") {
    return `\`${fieldName}\` must be a boolean when provided`;
  }

  return value;
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

  const line = parsePositiveInteger(args.line, "line");
  if (typeof line === "string") {
    return line;
  }

  const column = parsePositiveInteger(args.column, "column");
  if (typeof column === "string") {
    return column;
  }

  const preview = parseOptionalBoolean(args.preview, "preview");
  if (typeof preview === "string") {
    return preview;
  }

  const preserveFocus = parseOptionalBoolean(args.preserveFocus, "preserveFocus");
  if (typeof preserveFocus === "string") {
    return preserveFocus;
  }

  return {
    path: filePath,
    line,
    column,
    preview,
    preserveFocus
  };
}

function parseOpenPathArgs(args: unknown): IdeOpenPathArgs | string {
  if (!isObjectRecord(args)) {
    return "`args` must be an object";
  }

  const targetPath = args.path;
  if (typeof targetPath !== "string" || targetPath.trim().length === 0) {
    return "`path` must be a non-empty string";
  }

  const line = parsePositiveInteger(args.line, "line");
  if (typeof line === "string") {
    return line;
  }

  const column = parsePositiveInteger(args.column, "column");
  if (typeof column === "string") {
    return column;
  }

  const preview = parseOptionalBoolean(args.preview, "preview");
  if (typeof preview === "string") {
    return preview;
  }

  const preserveFocus = parseOptionalBoolean(args.preserveFocus, "preserveFocus");
  if (typeof preserveFocus === "string") {
    return preserveFocus;
  }

  return {
    path: targetPath.trim(),
    line,
    column,
    preview,
    preserveFocus
  };
}

function parseOpenExternalArgs(args: unknown): IdeOpenExternalArgs | string {
  if (!isObjectRecord(args)) {
    return "`args` must be an object";
  }

  const url = args.url;
  if (typeof url !== "string" || url.trim().length === 0) {
    return "`url` must be a non-empty string";
  }

  return {
    url: url.trim()
  };
}

function parseOpenChatArgs(args: unknown): IdeOpenChatArgs | string {
  if (!isObjectRecord(args)) {
    return "`args` must be an object";
  }

  const query = args.query;
  if (typeof query !== "string" || query.length === 0) {
    return "`query` must be a non-empty string";
  }

  const isPartialQuery = parseOptionalBoolean(args.isPartialQuery, "isPartialQuery");
  if (typeof isPartialQuery === "string") {
    return isPartialQuery;
  }

  return {
    query,
    isPartialQuery
  };
}

function parseOpenDialogFilters(
  value: unknown
): Record<string, string[]> | string | undefined {
  if (value === undefined) {
    return undefined;
  }

  if (!isObjectRecord(value)) {
    return "`filters` must be an object when provided";
  }

  const normalized: Record<string, string[]> = {};
  for (const [rawLabel, rawExtensions] of Object.entries(value)) {
    const label = rawLabel.trim();
    if (!label) {
      return "Filter names must be non-empty strings";
    }

    if (!Array.isArray(rawExtensions)) {
      return `\`filters.${label}\` must be an array of extensions`;
    }

    const extensions: string[] = [];
    for (const rawExtension of rawExtensions) {
      if (typeof rawExtension !== "string") {
        return `\`filters.${label}\` entries must be strings`;
      }

      const trimmed = rawExtension.trim();
      if (!trimmed) {
        return `\`filters.${label}\` entries must be non-empty strings`;
      }

      const normalizedExtension = trimmed.replace(/^\.+/, "");
      if (!normalizedExtension) {
        return `\`filters.${label}\` entries must include an extension`;
      }

      extensions.push(normalizedExtension);
    }

    normalized[label] = extensions;
  }

  return normalized;
}

function parseOpenFilePickerArgs(args: unknown): IdeOpenFilePickerArgs | string {
  if (!isObjectRecord(args)) {
    return "`args` must be an object";
  }

  const canSelectFiles = parseOptionalBoolean(args.canSelectFiles, "canSelectFiles");
  if (typeof canSelectFiles === "string") {
    return canSelectFiles;
  }

  const canSelectFolders = parseOptionalBoolean(args.canSelectFolders, "canSelectFolders");
  if (typeof canSelectFolders === "string") {
    return canSelectFolders;
  }

  const canSelectMany = parseOptionalBoolean(args.canSelectMany, "canSelectMany");
  if (typeof canSelectMany === "string") {
    return canSelectMany;
  }

  const titleRaw = args.title;
  if (titleRaw !== undefined && typeof titleRaw !== "string") {
    return "`title` must be a string when provided";
  }

  const defaultPathRaw = args.defaultPath;
  if (defaultPathRaw !== undefined && typeof defaultPathRaw !== "string") {
    return "`defaultPath` must be a string when provided";
  }

  const filters = parseOpenDialogFilters(args.filters);
  if (typeof filters === "string") {
    return filters;
  }

  const normalizedDefaultPath = defaultPathRaw?.trim();
  if (normalizedDefaultPath && !path.isAbsolute(normalizedDefaultPath)) {
    return "`defaultPath` must be an absolute filesystem path";
  }

  const normalizedCanSelectFiles = canSelectFiles ?? true;
  const normalizedCanSelectFolders = canSelectFolders ?? false;
  if (!normalizedCanSelectFiles && !normalizedCanSelectFolders) {
    return "At least one of `canSelectFiles` or `canSelectFolders` must be true";
  }

  const normalizedTitle = titleRaw?.trim();
  return {
    canSelectFiles: normalizedCanSelectFiles,
    canSelectFolders: normalizedCanSelectFolders,
    canSelectMany: canSelectMany ?? false,
    title: normalizedTitle && normalizedTitle.length > 0 ? normalizedTitle : undefined,
    defaultPath: normalizedDefaultPath && normalizedDefaultPath.length > 0 ? normalizedDefaultPath : undefined,
    filters
  };
}

async function openFileInEditor(args: IdeOpenFileArgs): Promise<void> {
  if (!fs.existsSync(args.path)) {
    throw new Error("FILE_NOT_FOUND");
  }

  const resolvedPath = path.resolve(args.path);
  const document = await vscode.workspace.openTextDocument(vscode.Uri.file(resolvedPath));
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

  if (isTrackedHarmonyOpenInEditorTempPath(resolvedPath)) {
    trackedHarmonyTempFilePaths.add(resolvedPath);
  }
}

function resolvePathAgainstWorkspace(rawPath: string): string | undefined {
  const trimmedPath = rawPath.trim();
  if (!trimmedPath) {
    return undefined;
  }

  if (path.isAbsolute(trimmedPath)) {
    return fs.existsSync(trimmedPath) ? trimmedPath : undefined;
  }

  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  for (const folder of workspaceFolders) {
    const candidate = path.resolve(folder.uri.fsPath, trimmedPath);
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  return undefined;
}

async function openPathInIde(args: IdeOpenPathArgs): Promise<boolean> {
  const resolvedPath = resolvePathAgainstWorkspace(args.path);
  if (!resolvedPath) {
    return false;
  }

  let stats: fs.Stats;
  try {
    stats = fs.statSync(resolvedPath);
  } catch {
    return false;
  }

  if (stats.isFile()) {
    try {
      await openFileInEditor({
        path: resolvedPath,
        line: args.line,
        column: args.column,
        preview: args.preview,
        preserveFocus: args.preserveFocus
      });
      return true;
    } catch {
      return false;
    }
  }

  if (stats.isDirectory()) {
    const directoryUri = vscode.Uri.file(resolvedPath);
    if (!vscode.workspace.getWorkspaceFolder(directoryUri)) {
      return false;
    }

    try {
      await vscode.commands.executeCommand("revealInExplorer", directoryUri);
      return true;
    } catch {
      return false;
    }
  }

  return false;
}

async function openExternalUrl(args: IdeOpenExternalArgs): Promise<boolean> {
  let parsedUrl: URL;
  try {
    parsedUrl = new URL(args.url);
  } catch {
    return false;
  }

  if (parsedUrl.protocol !== "http:" && parsedUrl.protocol !== "https:") {
    return false;
  }

  try {
    return Boolean(await vscode.env.openExternal(vscode.Uri.parse(parsedUrl.toString())));
  } catch {
    return false;
  }
}

async function openChatInIde(args: IdeOpenChatArgs): Promise<boolean> {
  try {
    await vscode.commands.executeCommand("workbench.action.chat.open", {
      query: args.query,
      isPartialQuery: args.isPartialQuery ?? true
    });
    return true;
  } catch {
    return false;
  }
}

async function openFilePickerInIde(args: IdeOpenFilePickerArgs): Promise<IdeOpenFilePickerResult> {
  const options: vscode.OpenDialogOptions = {
    canSelectFiles: args.canSelectFiles ?? true,
    canSelectFolders: args.canSelectFolders ?? false,
    canSelectMany: args.canSelectMany ?? false
  };

  if (args.title) {
    options.title = args.title;
  }

  if (args.defaultPath) {
    options.defaultUri = vscode.Uri.file(args.defaultPath);
  }

  if (args.filters && Object.keys(args.filters).length > 0) {
    options.filters = args.filters;
  }

  const selection = await vscode.window.showOpenDialog(options);
  if (!selection || selection.length === 0) {
    return {
      canceled: true,
      paths: []
    };
  }

  return {
    canceled: false,
    paths: selection.map((uri) => uri.fsPath)
  };
}

async function handleHostBridgeInvoke(
  raw: unknown,
  webview: vscode.Webview,
  output: vscode.OutputChannel,
  hostInfo: IdeHostInfo,
  context: vscode.ExtensionContext
): Promise<void> {
  if (!isHostBridgeInvokeMessage(raw)) {
    return;
  }

  const { id, payload } = raw;
  const { action } = payload;

  if (action === "ide.getCapabilities") {
    output.appendLine("[host-bridge] ide.getCapabilities");
    await webview.postMessage(createHostBridgeCapabilitiesResult(id, hostInfo));
    return;
  }

  if (action === "ide.getHostInfo") {
    output.appendLine(`[host-bridge] ide.getHostInfo host=${hostInfo.host}`);
    await webview.postMessage(createHostBridgeHostInfoResult(id, hostInfo));
    return;
  }

  if (action === "ide.cursorMcp.addServer") {
    if (!isCursorMcpSupported(hostInfo)) {
      await webview.postMessage(
        createHostBridgeError(
          id,
          action,
          "UNSUPPORTED_HOST",
          "Cursor MCP integration is only available when running inside Cursor."
        )
      );
      return;
    }

    try {
      const cursorMcpApi = getCursorMcpApi();
      if (!cursorMcpApi) {
        throw new Error("Cursor MCP API is unavailable.");
      }

      await ensureBridgeStarted(context, output);
      cursorMcpApi.registerServer({
        name: HARMONY_CURSOR_MCP_SERVER_NAME,
        server: {
          url: harmonyMcpServerUrl()
        }
      });
      output.appendLine(`[host-bridge] ide.cursorMcp.addServer url=${harmonyMcpServerUrl()}`);
      await webview.postMessage(
        createHostBridgeCursorMcpStatusResult(id, action, {
          added: true
        })
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      output.appendLine(`[host-bridge:OPEN_FAILED] ${message}`);
      await webview.postMessage(createHostBridgeError(id, action, "OPEN_FAILED", message));
    }

    return;
  }

  if (action === "ide.cursorMcp.removeServer") {
    if (!isCursorMcpSupported(hostInfo)) {
      await webview.postMessage(
        createHostBridgeError(
          id,
          action,
          "UNSUPPORTED_HOST",
          "Cursor MCP integration is only available when running inside Cursor."
        )
      );
      return;
    }

    try {
      const cursorMcpApi = getCursorMcpApi();
      if (!cursorMcpApi) {
        throw new Error("Cursor MCP API is unavailable.");
      }

      cursorMcpApi.unregisterServer(HARMONY_CURSOR_MCP_SERVER_NAME);
      output.appendLine("[host-bridge] ide.cursorMcp.removeServer");
      await webview.postMessage(
        createHostBridgeCursorMcpStatusResult(id, action, {
          added: false
        })
      );
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      output.appendLine(`[host-bridge:OPEN_FAILED] ${message}`);
      await webview.postMessage(createHostBridgeError(id, action, "OPEN_FAILED", message));
    }

    return;
  }

  if (action === "ide.openFile") {
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

    return;
  }

  if (action === "ide.openPath") {
    const parsedArgs = parseOpenPathArgs(payload.args);
    if (typeof parsedArgs === "string") {
      await webview.postMessage(createHostBridgeError(id, action, "INVALID_ARGS", parsedArgs));
      return;
    }

    output.appendLine(`[host-bridge] ide.openPath path=${parsedArgs.path}`);
    const opened = await openPathInIde(parsedArgs);
    output.appendLine(`[host-bridge] ide.openPath opened=${opened}`);
    await webview.postMessage(createHostBridgeOpenPathResult(id, opened));
    return;
  }

  if (action === "ide.openChat") {
    const parsedArgs = parseOpenChatArgs(payload.args);
    if (typeof parsedArgs === "string") {
      await webview.postMessage(createHostBridgeError(id, action, "INVALID_ARGS", parsedArgs));
      return;
    }

    output.appendLine("[host-bridge] ide.openChat");
    const opened = await openChatInIde(parsedArgs);
    output.appendLine(`[host-bridge] ide.openChat opened=${opened}`);
    await webview.postMessage(createHostBridgeOpenChatResult(id, opened));
    return;
  }

  if (action === "ide.openFilePicker") {
    const parsedArgs = parseOpenFilePickerArgs(payload.args);
    if (typeof parsedArgs === "string") {
      await webview.postMessage(createHostBridgeError(id, action, "INVALID_ARGS", parsedArgs));
      return;
    }

    output.appendLine("[host-bridge] ide.openFilePicker");
    try {
      const result = await openFilePickerInIde(parsedArgs);
      output.appendLine(
        `[host-bridge] ide.openFilePicker canceled=${result.canceled} paths=${result.paths.length}`
      );
      await webview.postMessage(createHostBridgeOpenFilePickerResult(id, result));
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      output.appendLine(`[host-bridge:OPEN_FAILED] ${message}`);
      await webview.postMessage(createHostBridgeError(id, action, "OPEN_FAILED", message));
    }

    return;
  }

  const parsedArgs = parseOpenExternalArgs(payload.args);
  if (typeof parsedArgs === "string") {
    await webview.postMessage(createHostBridgeError(id, action, "INVALID_ARGS", parsedArgs));
    return;
  }

  output.appendLine(`[host-bridge] ide.openExternal url=${parsedArgs.url}`);
  const opened = await openExternalUrl(parsedArgs);
  output.appendLine(`[host-bridge] ide.openExternal opened=${opened}`);
  await webview.postMessage(createHostBridgeOpenExternalResult(id, opened));
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
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src ${webview.cspSource} data:; style-src ${webview.cspSource} 'unsafe-inline'; script-src 'nonce-${nonce}'; connect-src ${BRIDGE_WS_URL};" />
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

function createOpenHarmonyViewStatusBarItem(): vscode.StatusBarItem {
  const item = vscode.window.createStatusBarItem(
    "harmony.openHelperView.statusBar",
    vscode.StatusBarAlignment.Left,
    1_000
  );

  item.name = "Open Harmony Helper View";
  item.text = "Open Harmony Helper View";
  item.tooltip = "Open the Harmony Helper view.";
  item.command = OPEN_HARMONY_HELPER_VIEW_COMMAND_ID;

  return item;
}

async function openHarmonyHelperView(webviewView: vscode.WebviewView | undefined): Promise<void> {
  await vscode.commands.executeCommand(`workbench.view.extension.${HARMONY_PANEL_ID}`);

  if (webviewView) {
    webviewView.show(false);
  }
}

function cleanupTrackedHarmonyTempFiles(output?: vscode.OutputChannel): void {
  let removedCount = 0;
  let failedCount = 0;

  for (const filePath of trackedHarmonyTempFilePaths) {
    try {
      fs.unlinkSync(filePath);
      removedCount += 1;
    } catch (error) {
      const code = (error as NodeJS.ErrnoException).code;
      if (code === "ENOENT") {
        continue;
      }

      failedCount += 1;
      output?.appendLine(`[open-in-editor:cleanup] failed to remove ${filePath}: ${String(error)}`);
    }
  }

  trackedHarmonyTempFilePaths.clear();

  if (removedCount > 0 || failedCount > 0) {
    output?.appendLine(
      `[open-in-editor:cleanup] removed=${removedCount} failed=${failedCount}`
    );
  }
}

export function activate(context: vscode.ExtensionContext): void {
  const output = vscode.window.createOutputChannel("Harmony");
  extensionOutputChannel = output;
  const hostInfo = detectHostIdeInfo();
  const mediaRoot = vscode.Uri.joinPath(context.extensionUri, "media");
  let harmonyWebviewView: vscode.WebviewView | undefined;
  let hasOpenedHarmonyView = context.globalState.get<boolean>(HARMONY_VIEW_OPENED_STATE_KEY, false);
  const openHarmonyViewStatusBarItem = hasOpenedHarmonyView
    ? undefined
    : createOpenHarmonyViewStatusBarItem();

  output.appendLine(
    `[host] detected ide=${hostInfo.host} scheme=${hostInfo.uriScheme || "unknown"} appName=${hostInfo.appName || "unknown"} officialVsCode=${hostInfo.isOfficialVsCode}`
  );

  const markHarmonyViewOpened = async (): Promise<void> => {
    if (hasOpenedHarmonyView) {
      return;
    }

    hasOpenedHarmonyView = true;
    openHarmonyViewStatusBarItem?.hide();
    await context.globalState.update(HARMONY_VIEW_OPENED_STATE_KEY, true);
  };

  const openHarmonyViewCommand = vscode.commands.registerCommand(
    OPEN_HARMONY_HELPER_VIEW_COMMAND_ID,
    async () => {
      await openHarmonyHelperView(harmonyWebviewView);
    }
  );

  const webviewProvider = vscode.window.registerWebviewViewProvider(
    HARMONY_VIEW_ID,
    {
      resolveWebviewView: async (webviewView) => {
        harmonyWebviewView = webviewView;
        await markHarmonyViewOpened();
        webviewView.webview.options = {
          enableScripts: true,
          localResourceRoots: [mediaRoot]
        };

        const onDidReceiveMessage = webviewView.webview.onDidReceiveMessage((raw) => {
          void handleHostBridgeInvoke(raw, webviewView.webview, output, hostInfo, context);
        });
        const onDidDispose = webviewView.onDidDispose(() => {
          if (harmonyWebviewView === webviewView) {
            harmonyWebviewView = undefined;
          }
        });
        context.subscriptions.push(onDidReceiveMessage, onDidDispose);

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

  openHarmonyViewStatusBarItem?.show();

  context.subscriptions.push(openHarmonyViewCommand, webviewProvider, output);

  if (openHarmonyViewStatusBarItem) {
    context.subscriptions.push(openHarmonyViewStatusBarItem);
  }

  context.subscriptions.push({
    dispose: () => {
      cleanupTrackedHarmonyTempFiles(output);
      stopBridgeProcess(output);
    }
  });
}

export function deactivate(): void {
  cleanupTrackedHarmonyTempFiles(extensionOutputChannel);

  if (bridgeProcess && bridgeProcess.exitCode === null) {
    bridgeProcess.kill();
  }

  bridgeProcess = undefined;
  extensionOutputChannel = undefined;
}
