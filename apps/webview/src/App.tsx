import { HarmonyWebSocketClient, type ConnectionState, resolveBootstrap } from "@harmony/webview-bridge";
import { useEffect, useMemo, useState } from "react";
import { useHdcDeviceSelection } from "./features/hdc/useHdcDeviceSelection";

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const [state, setState] = useState<ConnectionState>("idle");
  const [client, setClient] = useState<HarmonyWebSocketClient>();

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

  const deviceSelection = useHdcDeviceSelection({
    client,
    connectionState: state
  });

  const isComboboxEnabled =
    state === "open" &&
    deviceSelection.isSupported &&
    !deviceSelection.isRefreshing &&
    deviceSelection.devices.length > 0;

  const placeholder = (() => {
    if (state !== "open") {
      return "Connecting...";
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

  return (
    <main className="app-shell app-shell-compact">
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
    </main>
  );
}
