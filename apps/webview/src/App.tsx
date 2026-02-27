import { HarmonyWebSocketClient, type ConnectionState, resolveBootstrap } from "@harmony/webview-bridge";
import { useEffect, useMemo, useState } from "react";
import { HdcSettingsDialog } from "./features/hdc/HdcSettingsDialog";
import { useHdcBinConfig } from "./features/hdc/useHdcBinConfig";
import { useHdcDeviceSelection } from "./features/hdc/useHdcDeviceSelection";

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const [state, setState] = useState<ConnectionState>("idle");
  const [client, setClient] = useState<HarmonyWebSocketClient>();
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    const websocketClient = new HarmonyWebSocketClient(bootstrap);
    setClient(websocketClient);

    const offStatus = websocketClient.onStatus(setState);

    websocketClient.connect();

    return () => {
      offStatus();
      websocketClient.dispose();
      setClient(undefined);
    };
  }, [bootstrap]);

  const hdcBinConfig = useHdcBinConfig({
    client,
    connectionState: state
  });

  const deviceSelection = useHdcDeviceSelection({
    client,
    connectionState: state,
    hdcAvailable: hdcBinConfig.available
  });

  useEffect(() => {
    if (state !== "open" || !hdcBinConfig.available) {
      return;
    }

    void deviceSelection.refresh();
  }, [state, hdcBinConfig.available, deviceSelection.refresh]);

  const isComboboxEnabled =
    state === "open" &&
    hdcBinConfig.available &&
    deviceSelection.isSupported &&
    !deviceSelection.isRefreshing &&
    deviceSelection.devices.length > 0;

  const placeholder = (() => {
    if (state !== "open") {
      return "Connecting...";
    }

    if (hdcBinConfig.loading) {
      return "Checking HDC...";
    }

    if (!hdcBinConfig.available) {
      return "HDC unavailable";
    }

    if (!deviceSelection.isSupported) {
      return "HDC unsupported in this host";
    }

    if (deviceSelection.isRefreshing || deviceSelection.status === "loading") {
      return "Loading devices...";
    }

    if (deviceSelection.status === "error") {
      return "Failed to load devices";
    }

    return "No devices found";
  })();

  const showMissingHdcTip =
    state === "open" && hdcBinConfig.supported && !hdcBinConfig.loading && !hdcBinConfig.available;

  return (
    <main className="app-shell app-shell-compact">
      <div className="top-bar">
        <select
          className="hdc-device-combobox"
          aria-label="HDC device selection"
          value={deviceSelection.selectedDevice ?? ""}
          disabled={!isComboboxEnabled}
          onChange={(event) => {
            deviceSelection.selectDevice(event.target.value);
          }}
        >
          {deviceSelection.devices.length === 0 ? (
            <option value="">{placeholder}</option>
          ) : (
            deviceSelection.devices.map((device) => (
              <option key={device} value={device}>
                {device}
              </option>
            ))
          )}
        </select>

        <div className="top-bar-right">
          {showMissingHdcTip ? (
            <span
              className="hdc-error-tip"
              title={hdcBinConfig.message ?? "HDC binary is not configured"}
            >
              HDC not configured
            </span>
          ) : null}

          <button
            type="button"
            className="settings-icon-button"
            aria-label="Open HDC settings"
            onClick={() => {
              setSettingsOpen(true);
            }}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M19.4 13a7.8 7.8 0 0 0 0-2l2-1.5a.5.5 0 0 0 .1-.7l-1.9-3.3a.5.5 0 0 0-.6-.2L16.7 6a7.4 7.4 0 0 0-1.8-1l-.3-2.5a.5.5 0 0 0-.5-.4h-3.8a.5.5 0 0 0-.5.4L9.5 5A7.4 7.4 0 0 0 7.7 6L5.3 5.3a.5.5 0 0 0-.6.2L2.8 8.8a.5.5 0 0 0 .1.7l2 1.5a7.8 7.8 0 0 0 0 2l-2 1.5a.5.5 0 0 0-.1.7l1.9 3.3a.5.5 0 0 0 .6.2l2.4-.7a7.4 7.4 0 0 0 1.8 1l.3 2.5a.5.5 0 0 0 .5.4h3.8a.5.5 0 0 0 .5-.4l.3-2.5a7.4 7.4 0 0 0 1.8-1l2.4.7a.5.5 0 0 0 .6-.2l1.9-3.3a.5.5 0 0 0-.1-.7L19.4 13zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z" />
            </svg>
          </button>
        </div>
      </div>

      <HdcSettingsDialog
        open={settingsOpen}
        loading={hdcBinConfig.loading}
        saving={hdcBinConfig.saving}
        customBinPath={hdcBinConfig.customBinPath}
        resolvedBinPath={hdcBinConfig.resolvedBinPath}
        source={hdcBinConfig.source}
        message={hdcBinConfig.message}
        onClose={() => {
          setSettingsOpen(false);
        }}
        onSave={async (path) => {
          await hdcBinConfig.saveCustomPath(path);
          await hdcBinConfig.refresh();
        }}
        onClear={async () => {
          await hdcBinConfig.clearCustomPath();
          await hdcBinConfig.refresh();
        }}
      />
    </main>
  );
}
