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

const MAX_WRITE_BYTES_PER_FRAME = 128 * 1024;

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
  const activeSubscriptionRef = useRef<{ subscriptionId: string; connectKey: string } | null>(null);
  const autoScrollRef = useRef(true);
  const manualPausedRef = useRef(false);

  const [status, setStatus] = useState<HilogStatus>("idle");
  const [manualPaused, setManualPaused] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [droppedCount, setDroppedCount] = useState(0);
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

        const { chunk, dropped } = message.payload.data;
        if (dropped > 0) {
          setDroppedCount((current) => current + dropped);
        }

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

      if (activeSubscription?.connectKey === desiredConnectKey) {
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
      setDroppedCount(0);
      setErrorMessage(undefined);
      setStatus("idle");

      try {
        const result = await client.invoke("hdc.hilog.subscribe", { connectKey: desiredConnectKey });
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
          connectKey: result.connectKey
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
  }, [client, shouldRun, selectedDevice, manualPaused, clearQueueAndTerminal]);

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

  const canStreamControl = connectionState === "open" && hdcAvailable && selectedDevice !== null;

  return (
    <section className="hilog-panel" aria-label="Hilog console">
      <div className="hilog-toolbar">
        <div className="hilog-toolbar-left">
          <span className={`hilog-status hilog-status-${status}`}>Status: {streamStatus}</span>
          <span className="hilog-device">Device: {selectedDevice ?? "none"}</span>
          <span className="hilog-dropped">Dropped: {droppedCount}</span>
        </div>

        <div className="hilog-toolbar-right">
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
