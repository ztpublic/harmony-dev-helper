import type { BinConfigSource, HdcBinConfigResult } from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { useCallback, useEffect, useMemo, useState } from "react";

interface UseHdcBinConfigArgs {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
}

interface UseHdcBinConfigResult {
  supported: boolean;
  available: boolean;
  loading: boolean;
  saving: boolean;
  customBinPath: string | null;
  resolvedBinPath: string | null;
  source: BinConfigSource;
  message?: string;
  refresh: () => Promise<void>;
  saveCustomPath: (path: string) => Promise<void>;
  clearCustomPath: () => Promise<void>;
}

const DEFAULT_STATE: Omit<UseHdcBinConfigResult, "refresh" | "saveCustomPath" | "clearCustomPath"> = {
  supported: false,
  available: false,
  loading: false,
  saving: false,
  customBinPath: null,
  resolvedBinPath: null,
  source: "none"
};

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

export function useHdcBinConfig({
  client,
  connectionState
}: UseHdcBinConfigArgs): UseHdcBinConfigResult {
  const [state, setState] = useState(DEFAULT_STATE);

  const setFromResult = useCallback((result: HdcBinConfigResult) => {
    setState((current) => ({
      ...current,
      available: result.available,
      customBinPath: result.customBinPath,
      resolvedBinPath: result.resolvedBinPath,
      source: result.source,
      message: result.message
    }));
  }, []);

  const refresh = useCallback(async () => {
    if (!client || connectionState !== "open") {
      return;
    }

    setState((current) => ({ ...current, loading: true, message: undefined }));

    try {
      const result = await client.invoke("hdc.getBinConfig", {});
      setFromResult(result);
      setState((current) => ({ ...current, loading: false }));
    } catch (error) {
      setState((current) => ({
        ...current,
        loading: false,
        available: false,
        source: "none",
        message: toErrorMessage(error)
      }));
    }
  }, [client, connectionState, setFromResult]);

  const saveCustomPath = useCallback(
    async (path: string) => {
      if (!client || connectionState !== "open") {
        throw new Error("WebSocket is not connected");
      }

      setState((current) => ({ ...current, saving: true, message: undefined }));
      try {
        const result = await client.invoke("hdc.setBinPath", { binPath: path });
        setFromResult(result);
      } catch (error) {
        const message = toErrorMessage(error);
        setState((current) => ({ ...current, message }));
        throw new Error(message);
      } finally {
        setState((current) => ({ ...current, saving: false }));
      }
    },
    [client, connectionState, setFromResult]
  );

  const clearCustomPath = useCallback(async () => {
    if (!client || connectionState !== "open") {
      throw new Error("WebSocket is not connected");
    }

    setState((current) => ({ ...current, saving: true, message: undefined }));
    try {
      const result = await client.invoke("hdc.setBinPath", { binPath: null });
      setFromResult(result);
    } catch (error) {
      const message = toErrorMessage(error);
      setState((current) => ({ ...current, message }));
      throw new Error(message);
    } finally {
      setState((current) => ({ ...current, saving: false }));
    }
  }, [client, connectionState, setFromResult]);

  useEffect(() => {
    let cancelled = false;

    const initialize = async () => {
      if (!client || connectionState !== "open") {
        setState(DEFAULT_STATE);
        return;
      }

      setState((current) => ({ ...current, loading: true, message: undefined }));

      try {
        const capability = await client.invoke("host.getCapabilities", {});
        if (cancelled) {
          return;
        }

        if (!capability.capabilities["hdc.getBinConfig"] || !capability.capabilities["hdc.setBinPath"]) {
          setState({
            ...DEFAULT_STATE,
            supported: false,
            message: "This host does not support HDC binary settings."
          });
          return;
        }

        setState((current) => ({ ...current, supported: true }));

        const result = await client.invoke("hdc.getBinConfig", {});
        if (cancelled) {
          return;
        }

        setState((current) => ({
          ...current,
          supported: true,
          available: result.available,
          customBinPath: result.customBinPath,
          resolvedBinPath: result.resolvedBinPath,
          source: result.source,
          message: result.message,
          loading: false
        }));
      } catch (error) {
        if (cancelled) {
          return;
        }

        setState((current) => ({
          ...current,
          supported: false,
          available: false,
          source: "none",
          loading: false,
          message: toErrorMessage(error)
        }));
      }
    };

    void initialize();

    return () => {
      cancelled = true;
    };
  }, [client, connectionState]);

  return useMemo(
    () => ({
      ...state,
      refresh,
      saveCustomPath,
      clearCustomPath
    }),
    [state, refresh, saveCustomPath, clearCustomPath]
  );
}
