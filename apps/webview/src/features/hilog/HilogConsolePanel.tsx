import type { HdcHilogBatchEventData, HdcHilogStateEventData, HostMessage } from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { FitAddon } from "@xterm/addon-fit";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Terminal } from "xterm";
import "xterm/css/xterm.css";

interface HilogConsolePanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
  hdcAvailable: boolean;
  selectedDevice: string | null;
  active: boolean;
  historyLimit: number;
}

type HilogStatus = "idle" | "running" | "paused" | "error";
type HilogLevelCode = "D" | "I" | "W" | "E" | "F";

const MAX_WRITE_BYTES_PER_FRAME = 128 * 1024;
const HILOG_LEVEL_OPTIONS: ReadonlyArray<{ code: HilogLevelCode; label: string }> = [
  { code: "D", label: "Debug" },
  { code: "I", label: "Info" },
  { code: "W", label: "Warn" },
  { code: "E", label: "Error" },
  { code: "F", label: "Fatal" }
];
const DEFAULT_LEVEL_CODES = HILOG_LEVEL_OPTIONS.map((option) => option.code);

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

export function HilogConsolePanel({
  client,
  connectionState,
  hdcAvailable,
  selectedDevice,
  active,
  historyLimit
}: HilogConsolePanelProps) {
  const terminalContainerRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const queueRef = useRef<string[]>([]);
  const rafIdRef = useRef<number | null>(null);
  const writingRef = useRef(false);
  const activeSubscriptionRef = useRef<{
    subscriptionId: string;
    connectKey: string;
    levelFilter?: string;
  } | null>(null);
  const autoScrollRef = useRef(true);
  const manualPausedRef = useRef(false);
  const levelFilterRef = useRef<HTMLDivElement>(null);

  const [status, setStatus] = useState<HilogStatus>("idle");
  const [manualPaused, setManualPaused] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [selectedLevelCodes, setSelectedLevelCodes] = useState<HilogLevelCode[]>(() => [
    ...DEFAULT_LEVEL_CODES
  ]);
  const [isLevelDropdownOpen, setIsLevelDropdownOpen] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string>();

  autoScrollRef.current = autoScroll;
  manualPausedRef.current = manualPaused;

  const clearQueueAndTerminal = useCallback(() => {
    queueRef.current = [];

    if (rafIdRef.current !== null) {
      window.cancelAnimationFrame(rafIdRef.current);
      rafIdRef.current = null;
    }

    terminalRef.current?.clear();
  }, []);

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

      while (queueRef.current.length > 0) {
        const next = queueRef.current[0];
        if (bytes > 0 && bytes + next.length > MAX_WRITE_BYTES_PER_FRAME) {
          break;
        }

        queueRef.current.shift();
        bytes += next.length;
        chunk += next;
      }

      if (!chunk) {
        return;
      }

      writingRef.current = true;
      terminal.write(chunk, () => {
        writingRef.current = false;

        if (autoScrollRef.current) {
          terminal.scrollToBottom();
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

      observer?.disconnect();
      window.removeEventListener("resize", fit);
      terminal.dispose();
      terminalRef.current = null;
      fitAddonRef.current = null;
      queueRef.current = [];
    };
  }, []);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }

    terminal.options.scrollback = historyLimit;
  }, [historyLimit]);

  useEffect(() => {
    if (!isLevelDropdownOpen) {
      return;
    }

    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || levelFilterRef.current?.contains(target)) {
        return;
      }

      setIsLevelDropdownOpen(false);
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsLevelDropdownOpen(false);
      }
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [isLevelDropdownOpen]);

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

        queueRef.current.push(chunk);
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
  }, [client, scheduleFlush]);

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
        activeSubscription.levelFilter === desiredLevelFilter
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

      clearQueueAndTerminal();
      setErrorMessage(undefined);
      setStatus("idle");

      try {
        const result = await client.invoke("hdc.hilog.subscribe", {
          connectKey: desiredConnectKey,
          ...(desiredLevelFilter ? { level: desiredLevelFilter } : {})
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
          ...(desiredLevelFilter ? { levelFilter: desiredLevelFilter } : {})
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
    clearQueueAndTerminal,
    desiredLevelFilter
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
              onClick={() => {
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
              clearQueueAndTerminal();
            }}
          >
            Clear
          </button>

          <label className="hilog-toggle" htmlFor="hilog-autoscroll">
            <input
              id="hilog-autoscroll"
              type="checkbox"
              checked={autoScroll}
              onChange={(event) => {
                setAutoScroll(event.target.checked);
              }}
            />
            Auto-scroll
          </label>
        </div>
      </div>

      {errorMessage ? <p className="hilog-error">{errorMessage}</p> : null}

      <div className="hilog-terminal-frame">
        <div ref={terminalContainerRef} className="hilog-terminal" />
      </div>
    </section>
  );
}
