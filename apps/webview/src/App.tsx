import { HarmonyWebSocketClient, type ConnectionState, resolveBootstrap } from "@harmony/webview-bridge";
import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { MainTabbedPanel } from "./components/MainTabbedPanel";
import { DeviceFileExplorerPanel } from "./features/filesystem/DeviceFileExplorerPanel";
import { useHdcBinConfig } from "./features/hdc/useHdcBinConfig";
import { useHdcDeviceSelection } from "./features/hdc/useHdcDeviceSelection";
import { HilogConsolePanel } from "./features/hilog/HilogConsolePanel";
import {
  DEFAULT_MAIN_PANEL_TAB_ID,
  MAIN_PANEL_TABS,
  persistMainTab,
  readPersistedMainTab,
  type MainPanelTabId
} from "./features/mainPanel/mainPanelTabs";
import { SettingsDialog } from "./features/settings/SettingsDialog";
import {
  persistAppSettings,
  readAppSettings,
  type AppSettings,
  type AppTheme
} from "./features/settings/appSettings";

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const [state, setState] = useState<ConnectionState>("idle");
  const [client, setClient] = useState<HarmonyWebSocketClient>();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeMainTab, setActiveMainTab] = useState<MainPanelTabId>(() =>
    readPersistedMainTab(bootstrap.host, DEFAULT_MAIN_PANEL_TAB_ID)
  );
  const [appSettings, setAppSettings] = useState<AppSettings>(() => readAppSettings(bootstrap.host));

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

  useEffect(() => {
    setActiveMainTab(readPersistedMainTab(bootstrap.host, DEFAULT_MAIN_PANEL_TAB_ID));
    setAppSettings(readAppSettings(bootstrap.host));
  }, [bootstrap.host]);

  useEffect(() => {
    persistMainTab(bootstrap.host, activeMainTab);
  }, [bootstrap.host, activeMainTab]);

  useEffect(() => {
    persistAppSettings(bootstrap.host, appSettings);
  }, [bootstrap.host, appSettings]);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }

    document.documentElement.dataset.theme = appSettings.theme;
    document.documentElement.style.colorScheme = appSettings.theme;
  }, [appSettings.theme]);

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

  const saveHilogHistoryLimit = useCallback((limit: number) => {
    setAppSettings((current) => ({
      ...current,
      hilogHistoryLimit: limit
    }));
  }, []);

  const saveTheme = useCallback((theme: AppTheme) => {
    setAppSettings((current) => ({
      ...current,
      theme
    }));
  }, []);

  const mainTabPanels: Record<MainPanelTabId, ReactNode> = {
    hilog: (
      <HilogConsolePanel
        client={client}
        connectionState={state}
        hdcAvailable={hdcBinConfig.available}
        selectedDevice={deviceSelection.selectedDevice}
        active={activeMainTab === "hilog"}
        historyLimit={appSettings.hilogHistoryLimit}
        theme={appSettings.theme}
      />
    ),
    fileExplorer: (
      <DeviceFileExplorerPanel
        client={client}
        connectionState={state}
        hdcAvailable={hdcBinConfig.available}
        selectedDevice={deviceSelection.selectedDevice}
      />
    )
  };

  return (
    <main className="app-shell app-shell-compact">
      <MainTabbedPanel
        tabs={MAIN_PANEL_TABS}
        activeTabId={activeMainTab}
        onTabChange={setActiveMainTab}
        panels={mainTabPanels}
        headerRight={
          <>
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
                    {deviceSelection.deviceLabels[device] ?? device}
                  </option>
                ))
              )}
            </select>

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
              aria-label="Open settings"
              onClick={() => {
                setSettingsOpen(true);
              }}
            >
              <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                <path d="M19.4 13a7.8 7.8 0 0 0 0-2l2-1.5a.5.5 0 0 0 .1-.7l-1.9-3.3a.5.5 0 0 0-.6-.2L16.7 6a7.4 7.4 0 0 0-1.8-1l-.3-2.5a.5.5 0 0 0-.5-.4h-3.8a.5.5 0 0 0-.5.4L9.5 5A7.4 7.4 0 0 0 7.7 6L5.3 5.3a.5.5 0 0 0-.6.2L2.8 8.8a.5.5 0 0 0 .1.7l2 1.5a7.8 7.8 0 0 0 0 2l-2 1.5a.5.5 0 0 0-.1.7l1.9 3.3a.5.5 0 0 0 .6.2l2.4-.7a7.4 7.4 0 0 0 1.8 1l.3 2.5a.5.5 0 0 0 .5.4h3.8a.5.5 0 0 0 .5-.4l.3-2.5a7.4 7.4 0 0 0 1.8-1l2.4.7a.5.5 0 0 0 .6-.2l1.9-3.3a.5.5 0 0 0-.1-.7L19.4 13zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z" />
              </svg>
            </button>
          </>
        }
      />

      <SettingsDialog
        open={settingsOpen}
        loading={hdcBinConfig.loading}
        saving={hdcBinConfig.saving}
        customBinPath={hdcBinConfig.customBinPath}
        resolvedBinPath={hdcBinConfig.resolvedBinPath}
        source={hdcBinConfig.source}
        message={hdcBinConfig.message}
        hilogHistoryLimit={appSettings.hilogHistoryLimit}
        theme={appSettings.theme}
        onClose={() => {
          setSettingsOpen(false);
        }}
        onSaveHdcPath={async (path) => {
          await hdcBinConfig.saveCustomPath(path);
          await hdcBinConfig.refresh();
        }}
        onClearHdcPath={async () => {
          await hdcBinConfig.clearCustomPath();
          await hdcBinConfig.refresh();
        }}
        onSaveHilogHistoryLimit={saveHilogHistoryLimit}
        onSaveTheme={saveTheme}
      />
    </main>
  );
}
