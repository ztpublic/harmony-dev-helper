import type { HostCapabilities } from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DeviceSelectionState } from "./types";

interface UseHdcDeviceSelectionArgs {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
  hdcAvailable?: boolean;
  pollMs?: number;
}

interface UseHdcDeviceSelectionResult extends DeviceSelectionState {
  refresh: () => Promise<void>;
  selectDevice: (connectKey: string) => void;
}

interface ListTargetsOptions {
  silent?: boolean;
}

const DEFAULT_POLL_MS = 5_000;

function haveSameTargets(currentTargets: string[], nextTargets: string[]): boolean {
  if (currentTargets.length !== nextTargets.length) {
    return false;
  }

  for (let index = 0; index < currentTargets.length; index += 1) {
    if (currentTargets[index] !== nextTargets[index]) {
      return false;
    }
  }

  return true;
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

export function useHdcDeviceSelection({
  client,
  connectionState,
  hdcAvailable,
  pollMs = DEFAULT_POLL_MS
}: UseHdcDeviceSelectionArgs): UseHdcDeviceSelectionResult {
  const [capabilities, setCapabilities] = useState<HostCapabilities | null>(null);
  const [status, setStatus] = useState<DeviceSelectionState["status"]>("idle");
  const [devices, setDevices] = useState<string[]>([]);
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string>();
  const isListTargetsInFlight = useRef(false);

  const isSupported = Boolean(capabilities?.["hdc.listTargets"]);

  const syncDevices = useCallback((nextTargets: string[]) => {
    setDevices((currentTargets) => (haveSameTargets(currentTargets, nextTargets) ? currentTargets : nextTargets));
    setSelectedDevice((current) => {
      if (nextTargets.length === 0) {
        return null;
      }

      if (current && nextTargets.includes(current)) {
        return current;
      }

      // Milestone default: always select the first available target.
      return nextTargets[0];
    });
  }, []);

  const listTargets = useCallback(async ({ silent = false }: ListTargetsOptions = {}) => {
    if (!client || connectionState !== "open") {
      return;
    }

    if (isListTargetsInFlight.current) {
      return;
    }

    isListTargetsInFlight.current = true;
    if (!silent) {
      setIsRefreshing(true);
    }

    try {
      const result = await client.invoke("hdc.listTargets", {});
      syncDevices(result.targets);
      setStatus("ready");
      setErrorMessage(undefined);
    } catch (error) {
      if (!silent) {
        setStatus("error");
        setErrorMessage(toErrorMessage(error));
      }
    } finally {
      if (!silent) {
        setIsRefreshing(false);
      }

      isListTargetsInFlight.current = false;
    }
  }, [client, connectionState, syncDevices]);

  const refresh = useCallback(async () => {
    if (!isSupported || hdcAvailable === false) {
      return;
    }

    await listTargets();
  }, [isSupported, hdcAvailable, listTargets]);

  useEffect(() => {
    let cancelled = false;

    const initialize = async () => {
      if (!client || connectionState !== "open") {
        setCapabilities(null);
        setStatus("idle");
        setDevices([]);
        setSelectedDevice(null);
        setErrorMessage(undefined);
        setIsRefreshing(false);
        return;
      }

      setStatus("loading");
      setErrorMessage(undefined);

      try {
        const capabilityResult = await client.invoke("host.getCapabilities", {});
        if (cancelled) {
          return;
        }

        setCapabilities(capabilityResult.capabilities);
        if (!capabilityResult.capabilities["hdc.listTargets"]) {
          setStatus("unsupported");
          setDevices([]);
          setSelectedDevice(null);
          return;
        }

        if (hdcAvailable === false) {
          setStatus("idle");
          setDevices([]);
          setSelectedDevice(null);
          return;
        }

        await listTargets();
      } catch (error) {
        if (cancelled) {
          return;
        }

        setStatus("error");
        setErrorMessage(toErrorMessage(error));
      }
    };

    void initialize();

    return () => {
      cancelled = true;
    };
  }, [client, connectionState, hdcAvailable, listTargets]);

  useEffect(() => {
    if (!client || connectionState !== "open" || !isSupported || hdcAvailable === false) {
      return;
    }

    const intervalId = window.setInterval(() => {
      void listTargets({ silent: true });
    }, pollMs);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [client, connectionState, isSupported, hdcAvailable, pollMs, listTargets]);

  const selectDevice = useCallback(
    (connectKey: string) => {
      setSelectedDevice((current) => {
        if (!devices.includes(connectKey)) {
          return current;
        }

        return connectKey;
      });
    },
    [devices]
  );

  return useMemo(
    () => ({
      capabilities,
      isSupported,
      status,
      devices,
      selectedDevice,
      isRefreshing,
      errorMessage,
      refresh,
      selectDevice
    }),
    [capabilities, isSupported, status, devices, selectedDevice, isRefreshing, errorMessage, refresh, selectDevice]
  );
}
