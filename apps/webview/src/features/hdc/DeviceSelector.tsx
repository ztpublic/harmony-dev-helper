import type { ConnectionState } from "@harmony/webview-bridge";
import type { DeviceLoadState } from "./types";

interface DeviceSelectorProps {
  host: string;
  connectionState: ConnectionState;
  status: DeviceLoadState;
  isSupported: boolean;
  devices: string[];
  selectedDevice: string | null;
  isRefreshing: boolean;
  errorMessage?: string;
  onRefresh: () => void;
  onSelectDevice: (connectKey: string) => void;
}

export function DeviceSelector({
  host,
  connectionState,
  status,
  isSupported,
  devices,
  selectedDevice,
  isRefreshing,
  errorMessage,
  onRefresh,
  onSelectDevice
}: DeviceSelectorProps) {
  const isBridgeOpen = connectionState === "open";
  const canInteract = isBridgeOpen && isSupported;

  return (
    <section className="panel device-panel" aria-live="polite">
      <p className="kicker">HDC</p>
      <h2>Device</h2>

      {!isBridgeOpen ? <p className="panel-message">Waiting for websocket connection.</p> : null}

      {isBridgeOpen && !isSupported ? (
        <p className="panel-message panel-message-warning">
          HDC actions are not available in this host ({host}).
        </p>
      ) : null}

      {canInteract ? (
        <>
          <div className="device-controls">
            <label htmlFor="device-select" className="device-label">
              Connected target
            </label>
            <div className="device-input-row">
              <select
                id="device-select"
                value={selectedDevice ?? ""}
                disabled={devices.length === 0 || isRefreshing}
                onChange={(event) => {
                  onSelectDevice(event.target.value);
                }}
              >
                {devices.length === 0 ? (
                  <option value="">No devices found</option>
                ) : (
                  devices.map((device) => (
                    <option key={device} value={device}>
                      {device}
                    </option>
                  ))
                )}
              </select>

              <button
                type="button"
                className="device-refresh"
                onClick={onRefresh}
                disabled={isRefreshing}
              >
                {isRefreshing ? "Refreshing..." : "Refresh"}
              </button>
            </div>
          </div>

          <p className="panel-message">
            {devices.length === 0
              ? "Connect a device and refresh the list."
              : `${devices.length} device(s) available.`}
          </p>
        </>
      ) : null}

      {status === "loading" ? <p className="panel-message">Loading HDC capabilities...</p> : null}

      {status === "error" && errorMessage ? (
        <p className="panel-message panel-message-error">{errorMessage}</p>
      ) : null}
    </section>
  );
}
