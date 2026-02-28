import type {
  HdcHilogBatchEventData,
  HdcHilogPidOption,
  HdcHilogStateEventData,
  HostMessage
} from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { FitAddon } from "@xterm/addon-fit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { AppTheme } from "../settings/appSettings";
import { Terminal } from "xterm";
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

const MAX_WRITE_BYTES_PER_FRAME = 128 * 1024;
const SEARCH_DEBOUNCE_MS = 250;
const ANSI_SEQUENCE_REGEX = /\x1b\[[0-9;]*m/g;
const DARK_HIGHLIGHT_PALETTE: HighlightPalette = {
  open: "\x1b[48;2;102;77;0m",
  close: "\x1b[49m"
};
const LIGHT_HIGHLIGHT_PALETTE: HighlightPalette = {
  open: "\x1b[48;2;255;224;130m",
  close: "\x1b[49m"
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

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
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

export function HilogConsolePanel({
  client,
  connectionState,
  hdcAvailable,
  selectedDevice,
  active,
  historyLimit,
  theme
}: HilogConsolePanelProps) {
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
      disableStdin: true
    });

    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(container);

    terminalRef.current = terminal;
    fitAddonRef.current = fitAddon;

    const fit = () => {
      fitAddon.fit();
    };

    fit();

    const scrollDisposable = terminal.onScroll(() => {
      if (writingRef.current) {
        writeScrollCountRef.current += 1;
      }

      if (!stickToEndRef.current) {
        return;
      }

      if (!isAtEnd(terminal)) {
        setStickToEnd(false);
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

      scrollDisposable.dispose();
      observer?.disconnect();
      window.removeEventListener("resize", fit);
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      viewVersionRef.current += 1;
      queueRef.current = [];
    };
  }, []);

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
    void refreshPidOptions();
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
  const pidFilterLabel = useMemo(() => {
    if (selectedPid === null) {
      return "PID: All";
    }

    return `PID: ${selectedPid}`;
  }, [selectedPid]);

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
              disabled={!canStreamControl || isPidLoading}
              onClick={() => {
                setIsLevelDropdownOpen(false);
                setIsPidDropdownOpen((current) => !current);
              }}
            >
              {pidFilterLabel}
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
                {pidOptions.map((option) => (
                  <button
                    key={option.pid}
                    type="button"
                    className={`hilog-pid-option${selectedPid === option.pid ? " hilog-pid-option-selected" : ""}`}
                    onClick={() => {
                      setSelectedPid(option.pid);
                      setIsPidDropdownOpen(false);
                    }}
                  >
                    {option.pid} — {option.command}
                  </button>
                ))}
              </div>
            ) : null}
          </div>

          <button
            type="button"
            className="hilog-button"
            disabled={!canStreamControl || isPidLoading}
            onClick={() => {
              void refreshPidOptions();
            }}
          >
            {isPidLoading ? "Refreshing PIDs..." : "Refresh PIDs"}
          </button>

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
    </section>
  );
}
