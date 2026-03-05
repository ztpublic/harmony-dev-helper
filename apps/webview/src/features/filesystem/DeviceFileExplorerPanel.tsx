import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { useEffect, useMemo, useState } from "react";
import { FileSystem } from "./FileSystem";
import { createHdcVirtualFileSystem } from "./hdcVirtualFileSystem";

interface DeviceFileExplorerPanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
  hdcAvailable: boolean;
  selectedDevice: string | null;
}

type CapabilitiesStatus = "idle" | "loading" | "ready" | "error";

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function ExplorerPlaceholder({
  message,
  errorMessage
}: {
  message: string;
  errorMessage?: string;
}) {
  return (
    <section className="panel file-system-device-panel" aria-label="Device file explorer">
      <p className="kicker">Files</p>
      <h2>File Explorer</h2>
      <p className="panel-message">{message}</p>
      {errorMessage ? <p className="panel-message panel-message-error">{errorMessage}</p> : null}
    </section>
  );
}

export function DeviceFileExplorerPanel({
  client,
  connectionState,
  hdcAvailable,
  selectedDevice
}: DeviceFileExplorerPanelProps) {
  const [capabilitiesStatus, setCapabilitiesStatus] = useState<CapabilitiesStatus>("idle");
  const [supportsFsList, setSupportsFsList] = useState(false);
  const [capabilityError, setCapabilityError] = useState<string>();

  useEffect(() => {
    let cancelled = false;

    if (!client || connectionState !== "open") {
      setCapabilitiesStatus("idle");
      setSupportsFsList(false);
      setCapabilityError(undefined);
      return;
    }

    const loadCapabilities = async () => {
      setCapabilitiesStatus("loading");
      setCapabilityError(undefined);

      try {
        const result = await client.invoke("host.getCapabilities", {});
        if (cancelled) {
          return;
        }

        setSupportsFsList(Boolean(result.capabilities["hdc.fs.list"]));
        setCapabilitiesStatus("ready");
      } catch (error) {
        if (cancelled) {
          return;
        }

        setSupportsFsList(false);
        setCapabilitiesStatus("error");
        setCapabilityError(toErrorMessage(error));
      }
    };

    void loadCapabilities();

    return () => {
      cancelled = true;
    };
  }, [client, connectionState]);

  const vfs = useMemo(() => {
    if (!client || !selectedDevice || !supportsFsList) {
      return null;
    }

    return createHdcVirtualFileSystem({
      client,
      connectKey: selectedDevice,
      includeHidden: true
    });
  }, [client, selectedDevice, supportsFsList]);

  if (connectionState !== "open") {
    return <ExplorerPlaceholder message="Waiting for websocket connection." />;
  }

  if (!hdcAvailable) {
    return (
      <ExplorerPlaceholder message="HDC is unavailable. Configure the HDC binary in Settings." />
    );
  }

  if (capabilitiesStatus === "loading" || capabilitiesStatus === "idle") {
    return <ExplorerPlaceholder message="Loading HDC capabilities..." />;
  }

  if (capabilitiesStatus === "error") {
    return (
      <ExplorerPlaceholder
        message="Failed to load host capabilities for File Explorer."
        errorMessage={capabilityError}
      />
    );
  }

  if (!supportsFsList) {
    return (
      <ExplorerPlaceholder message="This bridge version does not support device file explorer yet." />
    );
  }

  if (!selectedDevice) {
    return <ExplorerPlaceholder message="Select a device to browse files." />;
  }

  if (!vfs) {
    return <ExplorerPlaceholder message="File explorer is not ready." />;
  }

  return <FileSystem key={selectedDevice} vfs={vfs} rootPath="/" />;
}
