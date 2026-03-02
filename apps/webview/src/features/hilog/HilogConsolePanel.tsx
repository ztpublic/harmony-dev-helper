import type {
  HdcHilogBatchEventData,
  HdcHilogPidOption,
  HdcHilogStateEventData,
  HostMessage,
  IdeCapabilities
} from "@harmony/protocol";
import {
  createHostBridgeClient,
  resolveBootstrap,
  type ConnectionState,
  type HarmonyHostBridgeClient,
  type HarmonyWebSocketClient
} from "@harmony/webview-bridge";
import { FitAddon } from "@xterm/addon-fit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppTheme } from "../settings/appSettings";
import { Terminal, type ILink } from "xterm";
import "xterm/css/xterm.css";

interface HilogConsolePanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
  hdcAvailable: boolean;
  selectedDevice: string | null;
  active: boolean;
  historyLimit: number;
  theme: AppTheme;
}

type HilogStatus = "idle" | "running" | "paused" | "error";
type HilogLevelCode = "D" | "I" | "W" | "E" | "F";
type QueuedChunk = {
  text: string;
  version: number;
};
type LineHistoryBuffer = {
  entries: string[];
  start: number;
  size: number;
  capacity: number;
};
type AnsiToken = {
  kind: "ansi" | "text";
  value: string;
};
type HighlightPalette = {
  open: string;
  close: string;
};
type TerminalPalette = {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
};
type OpenableLinkTarget =
  | {
      kind: "url";
      url: string;
    }
  | {
      kind: "path";
      path: string;
      line?: number;
      column?: number;
    };
type DetectedOpenableLink = {
  startOffset: number;
  endOffsetExclusive: number;
  text: string;
  target: OpenableLinkTarget;
};
type WrappedLineSegment = {
  bufferLineIndex: number;
  text: string;
  startOffset: number;
  endOffset: number;
};
type WrappedLineGroup = {
  combinedText: string;
  segments: WrappedLineSegment[];
};
type TerminalContextMenuState = {
  x: number;
  y: number;
  canCopy: boolean;
};
type SelectionChatButtonState = {
  x: number;
  y: number;
  query: string;
};

const MAX_WRITE_BYTES_PER_FRAME = 128 * 1024;
const SEARCH_DEBOUNCE_MS = 250;
const PID_TRIGGER_MAX_LABEL_CHARS = 36;
const SELECTION_CHAT_BUTTON_OFFSET_X = 8;
const SELECTION_CHAT_BUTTON_OFFSET_Y = 6;
const SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN = 8;
const ANSI_SEQUENCE_REGEX = /\x1b\[[0-9;]*m/g;
const LINK_TOKEN_REGEX = /\S+/g;
const HTTP_LINK_IN_TOKEN_REGEX = /https?:\/\/[^\s]+/gi;
const LINK_PATH_WITH_LOCATION_SUFFIX_REGEX = /^(.*):(\d+)(?::(\d+))?$/;
const WINDOWS_ABSOLUTE_PATH_REGEX = /^[A-Za-z]:[\\/]/;
const EDGE_TRIMMABLE_CHARS = "'\"()[]{}<>,;";
const DARK_HIGHLIGHT_PALETTE: HighlightPalette = {
  open: "\x1b[48;2;102;77;0m",
  close: "\x1b[49m"
};
const LIGHT_HIGHLIGHT_PALETTE: HighlightPalette = {
  open: "\x1b[48;2;255;224;130m",
  close: "\x1b[49m"
};
const TERMINAL_THEME_BY_APP_THEME: Record<AppTheme, TerminalPalette> = {
  dark: {
    background: "#1f1f1f",
    foreground: "#cccccc",
    cursor: "#cccccc",
    cursorAccent: "#1f1f1f",
    selectionBackground: "rgba(204, 204, 204, 0.25)"
  },
  light: {
    background: "#ffffff",
    foreground: "#3b3b3b",
    cursor: "#3b3b3b",
    cursorAccent: "#ffffff",
    selectionBackground: "rgba(59, 59, 59, 0.2)"
  }
};
const HILOG_LEVEL_OPTIONS: ReadonlyArray<{ code: HilogLevelCode; label: string }> = [
  { code: "D", label: "Debug" },
  { code: "I", label: "Info" },
  { code: "W", label: "Warn" },
  { code: "E", label: "Error" },
  { code: "F", label: "Fatal" }
];
const DEFAULT_LEVEL_CODES = HILOG_LEVEL_OPTIONS.map((option) => option.code);

function normalizeHistoryCapacity(value: number): number {
  if (!Number.isFinite(value)) {
    return 0;
  }

  return Math.max(0, Math.floor(value));
}

function createLineHistoryBuffer(capacity: number): LineHistoryBuffer {
  const normalizedCapacity = normalizeHistoryCapacity(capacity);
  return {
    entries: new Array(normalizedCapacity),
    start: 0,
    size: 0,
    capacity: normalizedCapacity
  };
}

function pushLineToHistory(buffer: LineHistoryBuffer, line: string): void {
  if (buffer.capacity === 0) {
    return;
  }

  if (buffer.size < buffer.capacity) {
    const index = (buffer.start + buffer.size) % buffer.capacity;
    buffer.entries[index] = line;
    buffer.size += 1;
    return;
  }

  buffer.entries[buffer.start] = line;
  buffer.start = (buffer.start + 1) % buffer.capacity;
}

function pushLinesToHistory(buffer: LineHistoryBuffer, lines: readonly string[]): void {
  for (const line of lines) {
    pushLineToHistory(buffer, line);
  }
}

function snapshotLineHistory(buffer: LineHistoryBuffer): string[] {
  if (buffer.size === 0 || buffer.capacity === 0) {
    return [];
  }

  const out = new Array<string>(buffer.size);
  for (let idx = 0; idx < buffer.size; idx += 1) {
    const ringIndex = (buffer.start + idx) % buffer.capacity;
    out[idx] = buffer.entries[ringIndex] ?? "";
  }

  return out;
}

function clearLineHistory(buffer: LineHistoryBuffer): void {
  buffer.entries = new Array(buffer.capacity);
  buffer.start = 0;
  buffer.size = 0;
}

function resizeLineHistory(buffer: LineHistoryBuffer, capacity: number): void {
  const nextCapacity = normalizeHistoryCapacity(capacity);
  if (buffer.capacity === nextCapacity) {
    return;
  }

  const snapshot = snapshotLineHistory(buffer);
  buffer.entries = new Array(nextCapacity);
  buffer.start = 0;
  buffer.size = 0;
  buffer.capacity = nextCapacity;

  if (nextCapacity === 0) {
    return;
  }

  const startIndex = Math.max(0, snapshot.length - nextCapacity);
  pushLinesToHistory(buffer, snapshot.slice(startIndex));
}

function normalizeSearchQuery(value: string): string {
  return value.trim().toLowerCase();
}

function tokenizeAnsi(line: string): AnsiToken[] {
  const tokens: AnsiToken[] = [];
  let start = 0;
  let match = ANSI_SEQUENCE_REGEX.exec(line);

  while (match) {
    const ansiStart = match.index;
    const ansi = match[0];

    if (ansiStart > start) {
      tokens.push({
        kind: "text",
        value: line.slice(start, ansiStart)
      });
    }

    tokens.push({
      kind: "ansi",
      value: ansi
    });

    start = ansiStart + ansi.length;
    match = ANSI_SEQUENCE_REGEX.exec(line);
  }

  ANSI_SEQUENCE_REGEX.lastIndex = 0;

  if (start < line.length) {
    tokens.push({
      kind: "text",
      value: line.slice(start)
    });
  }

  return tokens;
}

function matchesSearchTokenWise(line: string, normalizedQuery: string): boolean {
  if (!normalizedQuery) {
    return true;
  }

  const tokens = tokenizeAnsi(line);
  return tokens.some(
    (token) => token.kind === "text" && token.value.toLowerCase().includes(normalizedQuery)
  );
}

function highlightTextToken(
  text: string,
  normalizedQuery: string,
  palette: HighlightPalette
): string {
  if (!normalizedQuery) {
    return text;
  }

  const lower = text.toLowerCase();
  let searchFrom = 0;
  let out = "";

  while (searchFrom < text.length) {
    const matchIndex = lower.indexOf(normalizedQuery, searchFrom);
    if (matchIndex === -1) {
      out += text.slice(searchFrom);
      break;
    }

    out += text.slice(searchFrom, matchIndex);
    const matchEnd = matchIndex + normalizedQuery.length;
    out += `${palette.open}${text.slice(matchIndex, matchEnd)}${palette.close}`;
    searchFrom = matchEnd;
  }

  return out;
}

function highlightLineTokenWise(
  line: string,
  normalizedQuery: string,
  palette: HighlightPalette
): string {
  if (!normalizedQuery) {
    return line;
  }

  const tokens = tokenizeAnsi(line);
  return tokens
    .map((token) => {
      if (token.kind === "ansi") {
        return token.value;
      }

      return highlightTextToken(token.value, normalizedQuery, palette);
    })
    .join("");
}

function splitChunkIntoLines(chunk: string, remainder: string): { lines: string[]; remainder: string } {
  const combined = remainder + chunk;
  if (!combined) {
    return {
      lines: [],
      remainder: ""
    };
  }

  const lines: string[] = [];
  let start = 0;

  for (let idx = 0; idx < combined.length; idx += 1) {
    if (combined.charCodeAt(idx) === 10) {
      lines.push(combined.slice(start, idx + 1));
      start = idx + 1;
    }
  }

  return {
    lines,
    remainder: combined.slice(start)
  };
}

function normalizeLevelCodes(codes: readonly HilogLevelCode[]): HilogLevelCode[] {
  const wanted = new Set(codes);
  return HILOG_LEVEL_OPTIONS.filter((option) => wanted.has(option.code)).map((option) => option.code);
}

function buildLevelFilterArg(codes: readonly HilogLevelCode[]): string | undefined {
  const normalized = normalizeLevelCodes(codes);
  if (normalized.length === 0 || normalized.length === HILOG_LEVEL_OPTIONS.length) {
    return undefined;
  }

  return normalized.join(",");
}

function truncateWithEllipsis(value: string, maxChars: number): string {
  if (maxChars <= 0 || value.length <= maxChars) {
    return value;
  }

  if (maxChars <= 3) {
    return value.slice(0, maxChars);
  }

  return `${value.slice(0, maxChars - 3)}...`;
}

function comparePidOptionsByCommand(a: HdcHilogPidOption, b: HdcHilogPidOption): number {
  const commandOrder = a.command.localeCompare(b.command, undefined, { sensitivity: "base" });
  if (commandOrder !== 0) {
    return commandOrder;
  }

  return a.pid - b.pid;
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") {
    return false;
  }

  return /mac/i.test(navigator.platform);
}

function hasPrimaryModifier(event: { metaKey: boolean; ctrlKey: boolean }): boolean {
  return isMacPlatform() ? event.metaKey : event.ctrlKey;
}

function fallbackCopyText(text: string): boolean {
  if (typeof document === "undefined" || !document.body) {
    return false;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";

  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();

  let copied = false;
  try {
    copied = document.execCommand("copy");
  } catch {
    copied = false;
  }

  document.body.removeChild(textarea);
  return copied;
}

async function copyTextToClipboard(text: string): Promise<boolean> {
  if (!text) {
    return false;
  }

  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // fall through to legacy fallback
    }
  }

  return fallbackCopyText(text);
}

function shouldActivateLink(event: MouseEvent): boolean {
  const primaryClick = event.button === 0 || event.which === 1 || event.buttons === 1;
  if (!primaryClick) {
    return false;
  }

  return hasPrimaryModifier(event);
}

function isEdgeTrimmableChar(char: string): boolean {
  return EDGE_TRIMMABLE_CHARS.includes(char);
}

function trimTokenEdges(token: string): { trimmed: string; startOffset: number; endOffset: number } {
  let start = 0;
  let end = token.length;

  while (start < end && isEdgeTrimmableChar(token.charAt(start))) {
    start += 1;
  }

  while (end > start && isEdgeTrimmableChar(token.charAt(end - 1))) {
    end -= 1;
  }

  return {
    trimmed: token.slice(start, end),
    startOffset: start,
    endOffset: token.length - end
  };
}

function parseHttpUrl(candidate: string): string | undefined {
  try {
    const parsed = new URL(candidate);
    if (parsed.protocol === "http:" || parsed.protocol === "https:") {
      return parsed.toString();
    }
  } catch {
    // invalid URL
  }

  return undefined;
}

function looksLikePathCandidate(candidate: string): boolean {
  return (
    candidate.startsWith("/") ||
    candidate.startsWith("./") ||
    candidate.startsWith("../") ||
    candidate.startsWith("~/") ||
    WINDOWS_ABSOLUTE_PATH_REGEX.test(candidate) ||
    candidate.includes("/") ||
    candidate.includes("\\")
  );
}

function splitPathAndLocationSuffix(candidate: string): {
  path: string;
  line?: number;
  column?: number;
} {
  const matched = LINK_PATH_WITH_LOCATION_SUFFIX_REGEX.exec(candidate);
  if (!matched || matched[1].length === 0) {
    return { path: candidate };
  }

  const line = Number.parseInt(matched[2], 10);
  const column = matched[3] ? Number.parseInt(matched[3], 10) : undefined;
  if (
    Number.isNaN(line) ||
    line <= 0 ||
    (column !== undefined && (Number.isNaN(column) || column <= 0))
  ) {
    return { path: candidate };
  }

  return {
    path: matched[1],
    line,
    ...(column !== undefined ? { column } : {})
  };
}

function detectOpenablePathTarget(token: string): OpenableLinkTarget | undefined {
  const withLocation = splitPathAndLocationSuffix(token);
  if (!looksLikePathCandidate(withLocation.path)) {
    return undefined;
  }

  return {
    kind: "path",
    path: withLocation.path,
    ...(withLocation.line !== undefined ? { line: withLocation.line } : {}),
    ...(withLocation.column !== undefined ? { column: withLocation.column } : {})
  };
}

function detectOpenableLinksInText(text: string): DetectedOpenableLink[] {
  const links: DetectedOpenableLink[] = [];
  let match = LINK_TOKEN_REGEX.exec(text);

  while (match) {
    const rawToken = match[0];
    const tokenStart = match.index;
    const trimmed = trimTokenEdges(rawToken);

    if (trimmed.trimmed.length > 0) {
      const tokenLogicalStart = tokenStart + trimmed.startOffset;
      let urlMatch = HTTP_LINK_IN_TOKEN_REGEX.exec(trimmed.trimmed);
      let foundUrl = false;

      while (urlMatch) {
        const rawUrl = urlMatch[0];
        const urlOffset = urlMatch.index;
        const urlTrimmed = trimTokenEdges(rawUrl);
        if (urlTrimmed.trimmed.length > 0) {
          const parsedUrl = parseHttpUrl(urlTrimmed.trimmed);
          if (parsedUrl) {
            const urlStartOffset = tokenLogicalStart + urlOffset + urlTrimmed.startOffset;
            const urlEndOffsetExclusive =
              tokenLogicalStart + urlOffset + rawUrl.length - urlTrimmed.endOffset;

            links.push({
              startOffset: urlStartOffset,
              endOffsetExclusive: urlEndOffsetExclusive,
              text: urlTrimmed.trimmed,
              target: {
                kind: "url",
                url: parsedUrl
              }
            });
            foundUrl = true;
          }
        }

        urlMatch = HTTP_LINK_IN_TOKEN_REGEX.exec(trimmed.trimmed);
      }

      HTTP_LINK_IN_TOKEN_REGEX.lastIndex = 0;

      if (foundUrl || trimmed.trimmed.includes("http://") || trimmed.trimmed.includes("https://")) {
        match = LINK_TOKEN_REGEX.exec(text);
        continue;
      }

      const pathTarget = detectOpenablePathTarget(trimmed.trimmed);
      if (pathTarget) {
        links.push({
          startOffset: tokenLogicalStart,
          endOffsetExclusive: tokenLogicalStart + trimmed.trimmed.length,
          text: trimmed.trimmed,
          target: pathTarget
        });
      }
    }

    match = LINK_TOKEN_REGEX.exec(text);
  }

  LINK_TOKEN_REGEX.lastIndex = 0;
  return links;
}

function readWrappedLineGroup(terminal: Terminal, bufferLineIndex: number): WrappedLineGroup | null {
  const activeBuffer = terminal.buffer.active;
  const lineCount = activeBuffer.length;

  if (bufferLineIndex < 0 || bufferLineIndex >= lineCount) {
    return null;
  }

  let startIndex = bufferLineIndex;
  while (startIndex > 0) {
    const line = activeBuffer.getLine(startIndex);
    if (!line?.isWrapped) {
      break;
    }
    startIndex -= 1;
  }

  const segments: WrappedLineSegment[] = [];
  let combinedText = "";
  let offset = 0;

  for (let idx = startIndex; idx < lineCount; idx += 1) {
    const line = activeBuffer.getLine(idx);
    if (!line) {
      break;
    }

    if (idx !== startIndex && !line.isWrapped) {
      break;
    }

    const text = line.translateToString(false);
    const startOffset = offset;
    offset += text.length;
    combinedText += text;
    segments.push({
      bufferLineIndex: idx,
      text,
      startOffset,
      endOffset: offset
    });
  }

  return {
    combinedText,
    segments
  };
}

function mapLinkToSegment(
  link: DetectedOpenableLink,
  segment: WrappedLineSegment
): { startColumn: number; endColumnExclusive: number } | null {
  const start = Math.max(link.startOffset, segment.startOffset);
  const end = Math.min(link.endOffsetExclusive, segment.endOffset);
  if (start >= end) {
    return null;
  }

  return {
    startColumn: start - segment.startOffset,
    endColumnExclusive: end - segment.startOffset
  };
}

function isHilogBatchMessage(message: HostMessage): message is HostMessage & {
  type: "event";
  payload: { name: "hdc.hilog.batch"; data: HdcHilogBatchEventData };
} {
  return message.type === "event" && message.payload.name === "hdc.hilog.batch";
}

function isHilogStateMessage(message: HostMessage): message is HostMessage & {
  type: "event";
  payload: { name: "hdc.hilog.state"; data: HdcHilogStateEventData };
} {
  return message.type === "event" && message.payload.name === "hdc.hilog.state";
}

function isAtEnd(terminal: Terminal): boolean {
  return terminal.buffer.active.viewportY >= terminal.buffer.active.baseY;
}

function resolveTerminalPalette(theme: AppTheme): TerminalPalette {
  return TERMINAL_THEME_BY_APP_THEME[theme];
}

export function HilogConsolePanel({
  client,
  connectionState,
  hdcAvailable,
  selectedDevice,
  active,
  historyLimit,
  theme
}: HilogConsolePanelProps) {
  const runtimeHostRef = useRef(resolveBootstrap(window).host);
  const hostBridgeRef = useRef<HarmonyHostBridgeClient | null>(null);
  if (!hostBridgeRef.current) {
    hostBridgeRef.current = createHostBridgeClient();
  }
  const hostCapabilitiesRef = useRef<IdeCapabilities | null>(null);
  const hostCapabilitiesRequestRef = useRef<Promise<IdeCapabilities | null> | null>(null);
  const terminalContainerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const queueRef = useRef<QueuedChunk[]>([]);
  const rafIdRef = useRef<number | null>(null);
  const writingRef = useRef(false);
  const writeScrollCountRef = useRef(0);
  const viewVersionRef = useRef(0);
  const remainderRef = useRef("");
  const lineHistoryRef = useRef<LineHistoryBuffer>(createLineHistoryBuffer(historyLimit));
  const normalizedSearchRef = useRef("");
  const highlightPaletteRef = useRef<HighlightPalette>(
    theme === "light" ? LIGHT_HIGHLIGHT_PALETTE : DARK_HIGHLIGHT_PALETTE
  );
  const activeSubscriptionRef = useRef<{
    subscriptionId: string;
    connectKey: string;
    levelFilter?: string;
    pidFilter?: number;
  } | null>(null);
  const stickToEndRef = useRef(true);
  const manualPausedRef = useRef(false);
  const levelFilterRef = useRef<HTMLDivElement>(null);
  const pidFilterRef = useRef<HTMLDivElement>(null);
  const contextMenuRef = useRef<HTMLDivElement>(null);
  const selectionChatButtonRef = useRef<HTMLButtonElement>(null);
  const pidRefreshRequestIdRef = useRef(0);

  const [status, setStatus] = useState<HilogStatus>("idle");
  const [manualPaused, setManualPaused] = useState(false);
  const [stickToEnd, setStickToEnd] = useState(true);
  const [selectedLevelCodes, setSelectedLevelCodes] = useState<HilogLevelCode[]>(() => [
    ...DEFAULT_LEVEL_CODES
  ]);
  const [searchInput, setSearchInput] = useState("");
  const [activeSearch, setActiveSearch] = useState("");
  const [isLevelDropdownOpen, setIsLevelDropdownOpen] = useState(false);
  const [isPidDropdownOpen, setIsPidDropdownOpen] = useState(false);
  const [pidOptions, setPidOptions] = useState<HdcHilogPidOption[]>([]);
  const [selectedPid, setSelectedPid] = useState<number | null>(null);
  const [isPidLoading, setIsPidLoading] = useState(false);
  const [pidError, setPidError] = useState<string>();
  const [errorMessage, setErrorMessage] = useState<string>();
  const [terminalContextMenu, setTerminalContextMenu] = useState<TerminalContextMenuState | null>(null);
  const [selectionChatButton, setSelectionChatButton] = useState<SelectionChatButtonState | null>(null);
  const [canOpenChat, setCanOpenChat] = useState(false);

  stickToEndRef.current = stickToEnd;
  manualPausedRef.current = manualPaused;
  const normalizedSearch = useMemo(() => normalizeSearchQuery(activeSearch), [activeSearch]);
  const highlightPalette = useMemo<HighlightPalette>(
    () => (theme === "light" ? LIGHT_HIGHLIGHT_PALETTE : DARK_HIGHLIGHT_PALETTE),
    [theme]
  );
  normalizedSearchRef.current = normalizedSearch;
  highlightPaletteRef.current = highlightPalette;

  const renderVisibleLine = useCallback((line: string): string | null => {
    const query = normalizedSearchRef.current;
    if (!query) {
      return line;
    }

    if (!matchesSearchTokenWise(line, query)) {
      return null;
    }

    return highlightLineTokenWise(line, query, highlightPaletteRef.current);
  }, []);

  const enqueueVisibleLines = useCallback(
    (lines: readonly string[]) => {
      if (lines.length === 0) {
        return;
      }

      const version = viewVersionRef.current;
      let chunk = "";
      let chunkBytes = 0;

      for (const line of lines) {
        if (chunkBytes > 0 && chunkBytes + line.length > MAX_WRITE_BYTES_PER_FRAME) {
          queueRef.current.push({
            text: chunk,
            version
          });
          chunk = line;
          chunkBytes = line.length;
          continue;
        }

        chunk += line;
        chunkBytes += line.length;
      }

      if (chunk) {
        queueRef.current.push({
          text: chunk,
          version
        });
      }
    },
    []
  );

  const clearVisibleOnly = useCallback(() => {
    viewVersionRef.current += 1;
    queueRef.current = [];

    if (rafIdRef.current !== null) {
      window.cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }

    terminalRef.current?.clear();
  }, []);

  const clearHistoryAndVisible = useCallback(() => {
    remainderRef.current = "";
    clearLineHistory(lineHistoryRef.current);
    clearVisibleOnly();
  }, [clearVisibleOnly]);

  const toggleLevelCode = useCallback((code: HilogLevelCode) => {
    setSelectedLevelCodes((current) => {
      if (current.includes(code)) {
        if (current.length === 1) {
          return current;
        }

        return current.filter((value) => value !== code);
      }

      return normalizeLevelCodes([...current, code]);
    });
  }, []);

  const refreshPidOptions = useCallback(async () => {
    if (
      !client ||
      connectionState !== "open" ||
      !hdcAvailable ||
      !selectedDevice
    ) {
      setPidOptions([]);
      setSelectedPid(null);
      setPidError(undefined);
      setIsPidLoading(false);
      return;
    }

    const requestId = pidRefreshRequestIdRef.current + 1;
    pidRefreshRequestIdRef.current = requestId;
    setIsPidLoading(true);

    try {
      const result = await client.invoke("hdc.hilog.listPids", {
        connectKey: selectedDevice
      }, {
        timeoutMs: 20_000
      });

      if (pidRefreshRequestIdRef.current !== requestId) {
        return;
      }

      setPidOptions(result.pids);
      setSelectedPid((current) => {
        if (current === null) {
          return null;
        }

        return result.pids.some((option) => option.pid === current) ? current : null;
      });
      setPidError(undefined);
    } catch (error) {
      if (pidRefreshRequestIdRef.current !== requestId) {
        return;
      }

      setSelectedPid(null);
      setPidError(toErrorMessage(error));
    } finally {
      if (pidRefreshRequestIdRef.current === requestId) {
        setIsPidLoading(false);
      }
    }
  }, [client, connectionState, hdcAvailable, selectedDevice]);

  const copySelectedTerminalText = useCallback(async () => {
    const terminal = terminalRef.current;
    const selection = terminal?.getSelection() ?? "";
    if (!selection) {
      return;
    }

    await copyTextToClipboard(selection);
  }, []);

  const handleContextMenuCopy = useCallback(() => {
    void copySelectedTerminalText();
    setTerminalContextMenu(null);
  }, [copySelectedTerminalText]);

  const handleContextMenuSelectAll = useCallback(() => {
    terminalRef.current?.selectAll();
    setTerminalContextMenu(null);
  }, []);

  const handleContextMenuClear = useCallback(() => {
    clearHistoryAndVisible();
    setTerminalContextMenu(null);
  }, [clearHistoryAndVisible]);

  const updateSelectionChatButton = useCallback((terminal: Terminal) => {
    const query = terminal.getSelection();
    const selectionRange = terminal.getSelectionPosition();
    if (!query || !selectionRange) {
      setSelectionChatButton(null);
      return;
    }

    const screenElement = terminal.element?.querySelector(".xterm-screen");
    if (!(screenElement instanceof HTMLElement)) {
      setSelectionChatButton(null);
      return;
    }

    const screenBounds = screenElement.getBoundingClientRect();
    const cellWidth = screenBounds.width / terminal.cols;
    const cellHeight = screenBounds.height / terminal.rows;
    if (!Number.isFinite(cellWidth) || !Number.isFinite(cellHeight) || cellWidth <= 0 || cellHeight <= 0) {
      setSelectionChatButton(null);
      return;
    }

    const viewportRow = selectionRange.end.y - 1 - terminal.buffer.active.viewportY;
    if (viewportRow < 0 || viewportRow >= terminal.rows) {
      setSelectionChatButton(null);
      return;
    }

    const endColumn = Math.min(Math.max(selectionRange.end.x, 1), terminal.cols);
    const maxAnchorX = Math.max(
      SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN,
      window.innerWidth - SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN
    );
    const maxAnchorY = Math.max(
      SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN,
      window.innerHeight - SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN
    );
    const nextX = Math.min(
      Math.max(
        screenBounds.left + endColumn * cellWidth + SELECTION_CHAT_BUTTON_OFFSET_X,
        SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN
      ),
      maxAnchorX
    );
    const nextY = Math.min(
      Math.max(
        screenBounds.top + (viewportRow + 1) * cellHeight + SELECTION_CHAT_BUTTON_OFFSET_Y,
        SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN
      ),
      maxAnchorY
    );

    setSelectionChatButton((current) => {
      if (current?.query === query && current.x === nextX && current.y === nextY) {
        return current;
      }

      return {
        x: nextX,
        y: nextY,
        query
      };
    });
  }, []);

  const handleSelectionAddToChat = useCallback(async () => {
    if (!selectionChatButton) {
      return;
    }

    try {
      await hostBridgeRef.current?.invoke("ide.openChat", {
        query: selectionChatButton.query,
        isPartialQuery: true
      });
    } catch {
      // no-op
    }
  }, [selectionChatButton]);

  const scheduleFlush = useCallback(() => {
    if (rafIdRef.current !== null) {
      return;
    }

    rafIdRef.current = window.requestAnimationFrame(() => {
      rafIdRef.current = null;

      const terminal = terminalRef.current;
      if (!terminal) {
        return;
      }

      if (writingRef.current) {
        scheduleFlush();
        return;
      }

      let bytes = 0;
      let chunk = "";
      let chunkVersion = viewVersionRef.current;

      while (queueRef.current.length > 0) {
        const next = queueRef.current[0];
        if (next.version !== viewVersionRef.current) {
          queueRef.current.shift();
          continue;
        }

        if (bytes > 0 && bytes + next.text.length > MAX_WRITE_BYTES_PER_FRAME) {
          break;
        }

        queueRef.current.shift();
        bytes += next.text.length;
        chunk += next.text;
        chunkVersion = next.version;
      }

      if (!chunk) {
        return;
      }

      const preWriteViewportY = terminal.buffer.active.viewportY;
      const preWriteBaseY = terminal.buffer.active.baseY;
      writeScrollCountRef.current = 0;
      writingRef.current = true;
      terminal.write(chunk, () => {
        writingRef.current = false;
        const scrollCountDuringWrite = writeScrollCountRef.current;
        writeScrollCountRef.current = 0;

        if (chunkVersion !== viewVersionRef.current) {
          terminal.clear();
        } else if (stickToEndRef.current) {
          terminal.scrollToBottom();
        } else {
          const postWriteBaseY = terminal.buffer.active.baseY;
          const baseGrowth = Math.max(0, postWriteBaseY - preWriteBaseY);
          const trimmedLineCount = Math.max(0, scrollCountDuringWrite - baseGrowth);
          const frozenViewportY = Math.max(0, preWriteViewportY - trimmedLineCount);
          terminal.scrollToLine(Math.min(frozenViewportY, postWriteBaseY));
        }

        if (queueRef.current.length > 0) {
          scheduleFlush();
        }
      });
    });
  }, []);

  const getHostCapabilities = useCallback(async (): Promise<IdeCapabilities | null> => {
    const hostBridge = hostBridgeRef.current;
    if (!hostBridge) {
      return null;
    }

    if (hostCapabilitiesRef.current) {
      return hostCapabilitiesRef.current;
    }

    if (hostCapabilitiesRequestRef.current) {
      return hostCapabilitiesRequestRef.current;
    }

    const request = hostBridge
      .getCapabilities()
      .then((capabilities) => {
        hostCapabilitiesRef.current = capabilities;
        return capabilities;
      })
      .catch(() => null)
      .finally(() => {
        hostCapabilitiesRequestRef.current = null;
      });

    hostCapabilitiesRequestRef.current = request;
    return request;
  }, []);

  const activateOpenableTarget = useCallback(
    async (target: OpenableLinkTarget, event: MouseEvent) => {
      const modifierActive = isMacPlatform() ? event.metaKey : event.ctrlKey;
      const debugEvent = {
        button: event.button,
        buttons: event.buttons,
        which: event.which,
        metaKey: event.metaKey,
        ctrlKey: event.ctrlKey,
        modifierActive
      };

      if (!shouldActivateLink(event)) {
        console.debug("[hilog-link] ignored click (modifier/button mismatch)", {
          target,
          ...debugEvent
        });
        return;
      }

      if (target.kind === "url") {
        const capabilities = await getHostCapabilities();
        const runtimeHost = runtimeHostRef.current;
        console.debug("[hilog-link] opening url", {
          url: target.url,
          runtimeHost,
          capabilities,
          ...debugEvent
        });

        if (capabilities?.["ide.openExternal"]) {
          try {
            const result = await hostBridgeRef.current?.invoke("ide.openExternal", { url: target.url });
            console.debug("[hilog-link] ide.openExternal result", {
              url: target.url,
              opened: result?.opened ?? false
            });
            return;
          } catch {
            console.debug("[hilog-link] ide.openExternal failed", {
              url: target.url
            });
          }
        }

        if (runtimeHost === "browser") {
          try {
            window.open(target.url, "_blank", "noopener,noreferrer");
          } catch {
            // no-op
          }
        } else {
          console.debug("[hilog-link] skip window.open fallback for non-browser host", {
            url: target.url,
            runtimeHost
          });
        }
        return;
      }

      const capabilities = await getHostCapabilities();
      if (!capabilities?.["ide.openPath"]) {
        return;
      }

      try {
        const result = await hostBridgeRef.current?.invoke("ide.openPath", {
          path: target.path,
          ...(target.line !== undefined ? { line: target.line } : {}),
          ...(target.column !== undefined ? { column: target.column } : {})
        });
        console.debug("[hilog-link] ide.openPath result", {
          path: target.path,
          line: target.line,
          column: target.column,
          opened: result?.opened ?? false
        });
      } catch {
        // no-op
      }
    },
    [getHostCapabilities]
  );

  useEffect(() => {
    let cancelled = false;

    void getHostCapabilities().then((capabilities) => {
      if (cancelled) {
        return;
      }

      setCanOpenChat(Boolean(capabilities?.["ide.openChat"]));
    });

    return () => {
      cancelled = true;
    };
  }, [getHostCapabilities]);

  useEffect(() => {
    if (!canOpenChat) {
      setSelectionChatButton(null);
    }
  }, [canOpenChat]);

  useEffect(() => {
    if (!canOpenChat) {
      return;
    }

    const terminal = terminalRef.current;
    if (!terminal?.hasSelection()) {
      return;
    }

    updateSelectionChatButton(terminal);
  }, [canOpenChat, updateSelectionChatButton]);

  useEffect(() => {
    return () => {
      hostCapabilitiesRef.current = null;
      hostCapabilitiesRequestRef.current = null;
      hostBridgeRef.current?.dispose();
      hostBridgeRef.current = null;
    };
  }, []);

  useEffect(() => {
    const container = terminalContainerRef.current;
    if (!container) {
      return;
    }

    const terminal = new Terminal({
      fontFamily: '"JetBrains Mono", "SF Mono", Menlo, monospace',
      fontSize: 12,
      scrollback: historyLimit,
      convertEol: true,
      allowTransparency: true,
      cursorBlink: false,
      disableStdin: true,
      theme: resolveTerminalPalette(theme)
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(container);
    const openTerminalContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      event.stopPropagation();
      setTerminalContextMenu({
        x: event.clientX,
        y: event.clientY,
        canCopy: terminal.hasSelection()
      });
    };
    const onTerminalMouseDown = (event: MouseEvent) => {
      if (event.button !== 2) {
        return;
      }

      openTerminalContextMenu(event);
    };
    const onTerminalContextMenu = (event: MouseEvent) => {
      openTerminalContextMenu(event);
    };

    container.addEventListener("mousedown", onTerminalMouseDown, true);
    container.addEventListener("contextmenu", onTerminalContextMenu, true);

    const linkProviderDisposable = terminal.registerLinkProvider({
      provideLinks: (bufferLineNumber, callback) => {
        const bufferLineIndex = bufferLineNumber - 1;
        const wrappedGroup = readWrappedLineGroup(terminal, bufferLineIndex);
        if (!wrappedGroup || wrappedGroup.combinedText.length === 0) {
          callback(undefined);
          return;
        }

        const logicalLinks = detectOpenableLinksInText(wrappedGroup.combinedText);
        if (logicalLinks.length === 0) {
          callback(undefined);
          return;
        }

        const currentSegment = wrappedGroup.segments.find(
          (segment) => segment.bufferLineIndex === bufferLineIndex
        );
        if (!currentSegment) {
          callback(undefined);
          return;
        }

        const links: ILink[] = [];
        for (const link of logicalLinks) {
          const mapped = mapLinkToSegment(link, currentSegment);
          if (!mapped) {
            continue;
          }

          links.push({
            range: {
              start: {
                x: mapped.startColumn + 1,
                y: bufferLineNumber
              },
              end: {
                x: mapped.endColumnExclusive + 1,
                y: bufferLineNumber
              }
            },
            text: link.text,
            decorations: {
              underline: true,
              pointerCursor: true
            },
            activate: (event) => {
              void activateOpenableTarget(link.target, event);
            }
          });
        }

        if (links.length === 0) {
          callback(undefined);
          return;
        }

        callback(links);
      }
    });

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    const fit = () => {
      fitAddon.fit();
    };

    fit();
    const updateSelectionButton = () => {
      if (!canOpenChat) {
        setSelectionChatButton(null);
        return;
      }

      updateSelectionChatButton(terminal);
    };

    const scrollDisposable = terminal.onScroll(() => {
      if (writingRef.current) {
        writeScrollCountRef.current += 1;
      }

      if (terminal.hasSelection()) {
        updateSelectionButton();
      }

      if (!stickToEndRef.current) {
        return;
      }

      if (!isAtEnd(terminal)) {
        setStickToEnd(false);
      }
    });
    const selectionDisposable = terminal.onSelectionChange(() => {
      updateSelectionButton();
      setTerminalContextMenu((current) => {
        if (!current) {
          return current;
        }

        const canCopy = terminal.hasSelection();
        if (current.canCopy === canCopy) {
          return current;
        }

        return {
          ...current,
          canCopy
        };
      });
    });
    const renderDisposable = terminal.onRender(() => {
      if (terminal.hasSelection()) {
        updateSelectionButton();
      }
    });
    const resizeDisposable = terminal.onResize(() => {
      if (terminal.hasSelection()) {
        updateSelectionButton();
      }
    });

    let observer: ResizeObserver | undefined;
    if (typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(() => {
        fit();
      });
      observer.observe(container);
    }

    window.addEventListener("resize", fit);

    return () => {
      if (rafIdRef.current !== null) {
        window.cancelAnimationFrame(rafIdRef.current);
        rafIdRef.current = null;
      }

      linkProviderDisposable.dispose();
      scrollDisposable.dispose();
      selectionDisposable.dispose();
      renderDisposable.dispose();
      resizeDisposable.dispose();
      container.removeEventListener("mousedown", onTerminalMouseDown, true);
      container.removeEventListener("contextmenu", onTerminalContextMenu, true);
      observer?.disconnect();
      window.removeEventListener("resize", fit);
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      setSelectionChatButton(null);
      viewVersionRef.current += 1;
      queueRef.current = [];
    };
  }, [activateOpenableTarget, canOpenChat, updateSelectionChatButton]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    terminal.options.theme = resolveTerminalPalette(theme);
  }, [theme]);

  const redrawFromHistory = useCallback(() => {
    clearVisibleOnly();

    const lines = snapshotLineHistory(lineHistoryRef.current);
    const visibleLines = lines
      .map(renderVisibleLine)
      .filter((line): line is string => line !== null);

    enqueueVisibleLines(visibleLines);
    scheduleFlush();
  }, [clearVisibleOnly, enqueueVisibleLines, renderVisibleLine, scheduleFlush]);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    terminal.options.scrollback = historyLimit;
    resizeLineHistory(lineHistoryRef.current, historyLimit);
    redrawFromHistory();
  }, [historyLimit, redrawFromHistory]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setActiveSearch(searchInput);
    }, SEARCH_DEBOUNCE_MS);

    return () => {
      window.clearTimeout(timeout);
    };
  }, [searchInput]);

  useEffect(() => {
    redrawFromHistory();
  }, [normalizedSearch, theme, redrawFromHistory]);

  useEffect(() => {
    if (!active || connectionState !== "open" || !hdcAvailable || selectedDevice === null) {
      pidRefreshRequestIdRef.current += 1;
      setPidOptions([]);
      setSelectedPid(null);
      setPidError(undefined);
      setIsPidLoading(false);
      setIsPidDropdownOpen(false);
      return;
    }

    setPidOptions([]);
    setSelectedPid(null);
    setPidError(undefined);
    setIsPidDropdownOpen(false);
  }, [active, connectionState, hdcAvailable, selectedDevice, refreshPidOptions]);

  useEffect(() => {
    if (!isLevelDropdownOpen && !isPidDropdownOpen) {
      return;
    }

    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (
        !target ||
        levelFilterRef.current?.contains(target) ||
        pidFilterRef.current?.contains(target)
      ) {
        return;
      }

      setIsLevelDropdownOpen(false);
      setIsPidDropdownOpen(false);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsLevelDropdownOpen(false);
        setIsPidDropdownOpen(false);
      }
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [isLevelDropdownOpen, isPidDropdownOpen]);

  useEffect(() => {
    if (!terminalContextMenu) {
      return;
    }

    const menuElement = contextMenuRef.current;
    if (!menuElement) {
      return;
    }

    const bounds = menuElement.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const margin = 8;
    let nextX = terminalContextMenu.x;
    let nextY = terminalContextMenu.y;

    if (nextX + bounds.width + margin > viewportWidth) {
      nextX = Math.max(margin, viewportWidth - bounds.width - margin);
    }

    if (nextY + bounds.height + margin > viewportHeight) {
      nextY = Math.max(margin, viewportHeight - bounds.height - margin);
    }

    if (nextX === terminalContextMenu.x && nextY === terminalContextMenu.y) {
      return;
    }

    setTerminalContextMenu((current) => {
      if (!current) {
        return current;
      }

      if (current.x === nextX && current.y === nextY) {
        return current;
      }

      return {
        ...current,
        x: nextX,
        y: nextY
      };
    });
  }, [terminalContextMenu]);

  useEffect(() => {
    if (!selectionChatButton) {
      return;
    }

    const buttonElement = selectionChatButtonRef.current;
    if (!buttonElement) {
      return;
    }

    const bounds = buttonElement.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const margin = SELECTION_CHAT_BUTTON_VIEWPORT_MARGIN;
    let nextX = selectionChatButton.x;
    let nextY = selectionChatButton.y;

    if (nextX + bounds.width + margin > viewportWidth) {
      nextX = Math.max(margin, viewportWidth - bounds.width - margin);
    }

    if (nextY + bounds.height + margin > viewportHeight) {
      nextY = Math.max(margin, viewportHeight - bounds.height - margin);
    }

    if (nextX === selectionChatButton.x && nextY === selectionChatButton.y) {
      return;
    }

    setSelectionChatButton((current) => {
      if (!current) {
        return current;
      }

      if (current.x === nextX && current.y === nextY) {
        return current;
      }

      return {
        ...current,
        x: nextX,
        y: nextY
      };
    });
  }, [selectionChatButton]);

  useEffect(() => {
    if (!terminalContextMenu) {
      return;
    }

    const closeContextMenu = () => {
      setTerminalContextMenu(null);
    };

    const onMouseDown = (event: MouseEvent) => {
      if (event.button === 2) {
        return;
      }

      const target = event.target as Node | null;
      if (!target || contextMenuRef.current?.contains(target)) {
        return;
      }

      closeContextMenu();
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeContextMenu();
      }
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", closeContextMenu);

    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", closeContextMenu);
    };
  }, [terminalContextMenu]);

  useEffect(() => {
    if (!client) {
      return;
    }

    return client.onMessage((message) => {
      const activeSubscription = activeSubscriptionRef.current;
      if (!activeSubscription) {
        return;
      }

      if (isHilogBatchMessage(message)) {
        if (message.payload.data.subscriptionId !== activeSubscription.subscriptionId) {
          return;
        }

        const { chunk } = message.payload.data;

        if (!chunk) {
          return;
        }

        const parsed = splitChunkIntoLines(chunk, remainderRef.current);
        remainderRef.current = parsed.remainder;
        if (parsed.lines.length === 0) {
          return;
        }

        pushLinesToHistory(lineHistoryRef.current, parsed.lines);

        const visibleLines = parsed.lines
          .map(renderVisibleLine)
          .filter((line): line is string => line !== null);

        enqueueVisibleLines(visibleLines);
        scheduleFlush();
        return;
      }

      if (!isHilogStateMessage(message)) {
        return;
      }

      if (message.payload.data.subscriptionId !== activeSubscription.subscriptionId) {
        return;
      }

      const { state, message: stateMessage } = message.payload.data;
      if (state === "started") {
        setStatus("running");
        setErrorMessage(undefined);
        return;
      }

      if (state === "stopped") {
        if (!manualPausedRef.current) {
          setStatus("idle");
        }
        return;
      }

      if (state === "error") {
        setStatus("error");
        setErrorMessage(stateMessage ?? "Hilog stream failed");
      }
    });
  }, [client, enqueueVisibleLines, renderVisibleLine, scheduleFlush]);

  const shouldRun =
    active &&
    connectionState === "open" &&
    hdcAvailable &&
    selectedDevice !== null &&
    !manualPaused;
  const desiredLevelFilter = useMemo(
    () => buildLevelFilterArg(selectedLevelCodes),
    [selectedLevelCodes]
  );
  const desiredPidFilter = useMemo(
    () => (selectedPid === null ? undefined : selectedPid),
    [selectedPid]
  );

  useEffect(() => {
    if (!client) {
      return;
    }

    let cancelled = false;

    const syncStream = async () => {
      const desiredConnectKey = shouldRun ? selectedDevice : null;
      const activeSubscription = activeSubscriptionRef.current;

      if (!desiredConnectKey) {
        if (activeSubscription) {
          try {
            await client.invoke("hdc.hilog.unsubscribe", {
              subscriptionId: activeSubscription.subscriptionId
            });
          } catch {
            // best effort
          }

          activeSubscriptionRef.current = null;
        }

        if (!cancelled) {
          setStatus(manualPaused ? "paused" : "idle");
        }
        return;
      }

      if (
        activeSubscription?.connectKey === desiredConnectKey &&
        activeSubscription.levelFilter === desiredLevelFilter &&
        activeSubscription.pidFilter === desiredPidFilter
      ) {
        return;
      }

      if (activeSubscription) {
        try {
          await client.invoke("hdc.hilog.unsubscribe", {
            subscriptionId: activeSubscription.subscriptionId
          });
        } catch {
          // best effort
        }

        activeSubscriptionRef.current = null;
      }

      if (cancelled) {
        return;
      }

      clearHistoryAndVisible();
      setErrorMessage(undefined);
      setStatus("idle");

      try {
        const result = await client.invoke("hdc.hilog.subscribe", {
          connectKey: desiredConnectKey,
          ...(desiredLevelFilter ? { level: desiredLevelFilter } : {}),
          ...(desiredPidFilter !== undefined ? { pid: desiredPidFilter } : {})
        });
        if (cancelled) {
          try {
            await client.invoke("hdc.hilog.unsubscribe", {
              subscriptionId: result.subscriptionId
            });
          } catch {
            // best effort
          }
          return;
        }

        activeSubscriptionRef.current = {
          subscriptionId: result.subscriptionId,
          connectKey: result.connectKey,
          ...(desiredLevelFilter ? { levelFilter: desiredLevelFilter } : {}),
          ...(desiredPidFilter !== undefined ? { pidFilter: desiredPidFilter } : {})
        };
        setStatus("running");
        setErrorMessage(undefined);
      } catch (error) {
        if (cancelled) {
          return;
        }

        setStatus("error");
        setErrorMessage(toErrorMessage(error));
      }
    };

    void syncStream();

    return () => {
      cancelled = true;
    };
  }, [
    client,
    shouldRun,
    selectedDevice,
    manualPaused,
    clearHistoryAndVisible,
    desiredLevelFilter,
    desiredPidFilter
  ]);

  useEffect(() => {
    return () => {
      const activeSubscription = activeSubscriptionRef.current;
      activeSubscriptionRef.current = null;

      if (activeSubscription && client) {
        void client
          .invoke("hdc.hilog.unsubscribe", {
            subscriptionId: activeSubscription.subscriptionId
          })
          .catch(() => {
            // best effort
          });
      }
    };
  }, [client]);

  const streamStatus = useMemo(() => {
    if (connectionState !== "open") {
      return "Disconnected";
    }

    if (!hdcAvailable) {
      return "HDC unavailable";
    }

    if (!selectedDevice) {
      return "No device";
    }

    if (manualPaused) {
      return "Paused";
    }

    if (status === "running") {
      return "Running";
    }

    if (status === "error") {
      return "Error";
    }

    return "Idle";
  }, [connectionState, hdcAvailable, selectedDevice, manualPaused, status]);

  const levelFilterLabel = useMemo(() => {
    if (selectedLevelCodes.length === HILOG_LEVEL_OPTIONS.length) {
      return "Level: All";
    }

    return `Level: ${selectedLevelCodes.length} selected`;
  }, [selectedLevelCodes]);
  const sortedPidOptions = useMemo(
    () => [...pidOptions].sort(comparePidOptionsByCommand),
    [pidOptions]
  );
  const selectedPidOption = useMemo(
    () => (selectedPid === null ? undefined : pidOptions.find((option) => option.pid === selectedPid)),
    [pidOptions, selectedPid]
  );
  const pidFilterLabel = useMemo(() => {
    if (selectedPid === null) {
      return "PID: All";
    }

    const command = selectedPidOption?.command ?? String(selectedPid);
    return `PID: ${truncateWithEllipsis(command, PID_TRIGGER_MAX_LABEL_CHARS)}`;
  }, [selectedPid, selectedPidOption]);

  const canStreamControl = connectionState === "open" && hdcAvailable && selectedDevice !== null;

  return (
    <section className="hilog-panel" aria-label="Hilog console">
      <div className="hilog-toolbar">
        <div className="hilog-toolbar-left">
          <span className={`hilog-status hilog-status-${status}`}>Status: {streamStatus}</span>
        </div>

        <div className="hilog-toolbar-right">
          <div className="hilog-level-filter" ref={levelFilterRef}>
            <button
              type="button"
              className="hilog-level-trigger"
              aria-haspopup="true"
              aria-expanded={isLevelDropdownOpen}
              disabled={!canStreamControl}
              onClick={() => {
                setIsPidDropdownOpen(false);
                setIsLevelDropdownOpen((current) => !current);
              }}
            >
              {levelFilterLabel}
              <span className="hilog-level-trigger-caret">v</span>
            </button>

            {isLevelDropdownOpen ? (
              <div className="hilog-level-menu" role="menu" aria-label="Hilog log level filters">
                {HILOG_LEVEL_OPTIONS.map((option) => {
                  const checked = selectedLevelCodes.includes(option.code);
                  const disableUncheck = checked && selectedLevelCodes.length === 1;

                  return (
                    <label key={option.code} className="hilog-level-option">
                      <input
                        type="checkbox"
                        checked={checked}
                        disabled={disableUncheck}
                        onChange={() => {
                          toggleLevelCode(option.code);
                        }}
                      />
                      <span>{option.label}</span>
                    </label>
                  );
                })}
              </div>
            ) : null}
          </div>

          <div className="hilog-pid-filter" ref={pidFilterRef}>
            <button
              type="button"
              className="hilog-pid-trigger"
              aria-haspopup="true"
              aria-expanded={isPidDropdownOpen}
              disabled={!canStreamControl}
              onClick={() => {
                setIsLevelDropdownOpen(false);
                setIsPidDropdownOpen((current) => {
                  const next = !current;
                  if (next) {
                    void refreshPidOptions();
                  }

                  return next;
                });
              }}
            >
              <span className="hilog-pid-trigger-label">{pidFilterLabel}</span>
              <span className="hilog-pid-trigger-caret">v</span>
            </button>

            {isPidDropdownOpen ? (
              <div className="hilog-pid-menu" role="menu" aria-label="Hilog PID filters">
                <button
                  type="button"
                  className={`hilog-pid-option${selectedPid === null ? " hilog-pid-option-selected" : ""}`}
                  onClick={() => {
                    setSelectedPid(null);
                    setIsPidDropdownOpen(false);
                  }}
                >
                  All
                </button>
                {sortedPidOptions.map((option) => (
                  <button
                    key={option.pid}
                    type="button"
                    className={`hilog-pid-option${selectedPid === option.pid ? " hilog-pid-option-selected" : ""}`}
                    onClick={() => {
                      setSelectedPid(option.pid);
                      setIsPidDropdownOpen(false);
                    }}
                  >
                    {option.command} — {option.pid}
                  </button>
                ))}
              </div>
            ) : null}
          </div>

          <div className="hilog-search">
            <input
              type="text"
              className="hilog-search-input"
              aria-label="Search hilog logs"
              placeholder="Search logs"
              spellCheck={false}
              autoCorrect="off"
              autoCapitalize="off"
              autoComplete="off"
              value={searchInput}
              onChange={(event) => {
                setSearchInput(event.target.value);
              }}
            />
            {searchInput ? (
              <button
                type="button"
                className="hilog-search-clear"
                aria-label="Clear log search"
                onClick={() => {
                  setSearchInput("");
                }}
              >
                x
              </button>
            ) : null}
          </div>

          <button
            type="button"
            className="hilog-button"
            aria-pressed={stickToEnd}
            onClick={() => {
              setStickToEnd((current) => {
                const next = !current;
                if (next) {
                  terminalRef.current?.scrollToBottom();
                }
                return next;
              });
            }}
          >
            Stick to end
          </button>

          <button
            type="button"
            className="hilog-button"
            disabled={!canStreamControl}
            onClick={() => {
              setManualPaused((current) => !current);
            }}
          >
            {manualPaused ? "Resume" : "Pause"}
          </button>

          <button
            type="button"
            className="hilog-button"
            onClick={() => {
              clearHistoryAndVisible();
            }}
          >
            Clear
          </button>
        </div>
      </div>

      {pidError ? <p className="hilog-pid-error">{pidError}</p> : null}
      {errorMessage ? <p className="hilog-error">{errorMessage}</p> : null}

      <div className="hilog-terminal-frame">
        <div ref={terminalContainerRef} className="hilog-terminal" />
      </div>

      {canOpenChat && selectionChatButton ? (
        <button
          ref={selectionChatButtonRef}
          type="button"
          className="hilog-selection-chat-button"
          style={{
            left: `${selectionChatButton.x}px`,
            top: `${selectionChatButton.y}px`
          }}
          onMouseDown={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
          onClick={() => {
            void handleSelectionAddToChat();
          }}
        >
          Add to Chat
        </button>
      ) : null}

      {terminalContextMenu ? (
        <div
          ref={contextMenuRef}
          className="hilog-context-menu"
          role="menu"
          style={{
            left: `${terminalContextMenu.x}px`,
            top: `${terminalContextMenu.y}px`
          }}
        >
          <button
            type="button"
            className="hilog-context-menu-item"
            role="menuitem"
            disabled={!terminalContextMenu.canCopy}
            onClick={handleContextMenuCopy}
          >
            Copy
          </button>
          <button
            type="button"
            className="hilog-context-menu-item"
            role="menuitem"
            onClick={handleContextMenuSelectAll}
          >
            Select all
          </button>
          <button
            type="button"
            className="hilog-context-menu-item"
            role="menuitem"
            onClick={handleContextMenuClear}
          >
            Clear console
          </button>
        </div>
      ) : null}
    </section>
  );
}
