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
const DEVICE_LABEL_RETRY_MS = 30_000;
const PRODUCT_NAME_PARAM = "const.product.name";
const PRODUCT_MODEL_PARAM = "const.product.model";
const PRODUCT_BRAND_PARAM = "const.product.brand";

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

function haveSameLabels(
  currentLabels: Record<string, string>,
  nextLabels: Record<string, string>
): boolean {
  const currentKeys = Object.keys(currentLabels);
  const nextKeys = Object.keys(nextLabels);

  if (currentKeys.length !== nextKeys.length) {
    return false;
  }

  for (const key of currentKeys) {
    if (currentLabels[key] !== nextLabels[key]) {
      return false;
    }
  }

  return true;
}

function cleanParameterValue(value: string | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed ? trimmed : undefined;
}

function buildDeviceLabel(connectKey: string, parameters: Record<string, string>): string {
  const name = cleanParameterValue(parameters[PRODUCT_NAME_PARAM]);
  const model = cleanParameterValue(parameters[PRODUCT_MODEL_PARAM]);
  const brand = cleanParameterValue(parameters[PRODUCT_BRAND_PARAM]);

  const primary = name ?? (brand && model ? `${brand} ${model}` : model ?? brand);
  if (!primary) {
    return connectKey;
  }

  if (name && model && name !== model) {
    return `${name} (${model}) · ${connectKey}`;
  }

  return `${primary} · ${connectKey}`;
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
  const [deviceLabels, setDeviceLabels] = useState<Record<string, string>>({});
  const [selectedDevice, setSelectedDevice] = useState<string | null>(null);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string>();
  const isListTargetsInFlight = useRef(false);
  const deviceLabelsRef = useRef<Record<string, string>>({});
  const deviceLabelFetchInFlight = useRef<Set<string>>(new Set());
  const deviceLabelFetchAttemptAt = useRef<Map<string, number>>(new Map());

  const isSupported = Boolean(capabilities?.["hdc.listTargets"]);
  const canGetDeviceParameters = Boolean(capabilities?.["hdc.getParameters"]);

  useEffect(() => {
    deviceLabelsRef.current = deviceLabels;
  }, [deviceLabels]);

  const syncDevices = useCallback((nextTargets: string[]) => {
    setDevices((currentTargets) => (haveSameTargets(currentTargets, nextTargets) ? currentTargets : nextTargets));
    setDeviceLabels((currentLabels) => {
      const nextLabels: Record<string, string> = {};
      for (const connectKey of nextTargets) {
        const label = currentLabels[connectKey];
        if (label) {
          nextLabels[connectKey] = label;
        }
      }

      return haveSameLabels(currentLabels, nextLabels) ? currentLabels : nextLabels;
    });
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

    const nextTargetSet = new Set(nextTargets);
    for (const connectKey of Array.from(deviceLabelFetchInFlight.current)) {
      if (!nextTargetSet.has(connectKey)) {
        deviceLabelFetchInFlight.current.delete(connectKey);
      }
    }

    for (const connectKey of Array.from(deviceLabelFetchAttemptAt.current.keys())) {
      if (!nextTargetSet.has(connectKey)) {
        deviceLabelFetchAttemptAt.current.delete(connectKey);
      }
    }
  }, []);

  const loadDeviceLabels = useCallback(
    async (targets: string[]) => {
      if (!client || connectionState !== "open" || !canGetDeviceParameters || targets.length === 0) {
        return;
      }

      const now = Date.now();
      const pending = targets.filter((connectKey) => {
        if (deviceLabelFetchInFlight.current.has(connectKey)) {
          return false;
        }

        const existingLabel = deviceLabelsRef.current[connectKey];
        if (existingLabel && existingLabel !== connectKey) {
          return false;
        }

        const lastAttempt = deviceLabelFetchAttemptAt.current.get(connectKey);
        return !lastAttempt || now - lastAttempt >= DEVICE_LABEL_RETRY_MS;
      });

      if (pending.length === 0) {
        return;
      }

      await Promise.allSettled(
        pending.map(async (connectKey) => {
          deviceLabelFetchInFlight.current.add(connectKey);
          deviceLabelFetchAttemptAt.current.set(connectKey, now);

          try {
            const result = await client.invoke("hdc.getParameters", { connectKey });
            const label = buildDeviceLabel(connectKey, result.parameters);
            setDeviceLabels((currentLabels) => {
              if (currentLabels[connectKey] === label) {
                return currentLabels;
              }

              return {
                ...currentLabels,
                [connectKey]: label
              };
            });
          } catch {
            // Keep fallback label and retry later.
          } finally {
            deviceLabelFetchInFlight.current.delete(connectKey);
          }
        })
      );
    },
    [client, connectionState, canGetDeviceParameters]
  );

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
      void loadDeviceLabels(result.targets);
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
  }, [client, connectionState, syncDevices, loadDeviceLabels]);

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
        setDeviceLabels({});
        setSelectedDevice(null);
        setErrorMessage(undefined);
        setIsRefreshing(false);
        deviceLabelFetchInFlight.current.clear();
        deviceLabelFetchAttemptAt.current.clear();
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
          setDeviceLabels({});
          setSelectedDevice(null);
          deviceLabelFetchInFlight.current.clear();
          deviceLabelFetchAttemptAt.current.clear();
          return;
        }

        if (hdcAvailable === false) {
          setStatus("idle");
          setDevices([]);
          setDeviceLabels({});
          setSelectedDevice(null);
          deviceLabelFetchInFlight.current.clear();
          deviceLabelFetchAttemptAt.current.clear();
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
      deviceLabels,
      selectedDevice,
      isRefreshing,
      errorMessage,
      refresh,
      selectDevice
    }),
    [
      capabilities,
      isSupported,
      status,
      devices,
      deviceLabels,
      selectedDevice,
      isRefreshing,
      errorMessage,
      refresh,
      selectDevice
    ]
  );
}
