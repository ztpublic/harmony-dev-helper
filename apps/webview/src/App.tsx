import {
  HarmonyWebSocketClient,
  createHostBridgeClient,
  type ConnectionState,
  type HarmonyHostBridgeClient,
  resolveBootstrap
} from "@harmony/webview-bridge";
import { type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { MainTabbedPanel } from "./components/MainTabbedPanel";
import { DeviceFileExplorerPanel } from "./features/filesystem/DeviceFileExplorerPanel";
import { useHdcBinConfig } from "./features/hdc/useHdcBinConfig";
import { useHdcDeviceSelection } from "./features/hdc/useHdcDeviceSelection";
import { HilogConsolePanel } from "./features/hilog/HilogConsolePanel";
import { McpToolsPanel } from "./features/mcp/McpToolsPanel";
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

const WINDOWS_ABSOLUTE_PATH_REGEX = /^[A-Za-z]:[\\/]/;
const WINDOWS_UNC_PATH_REGEX = /^\\\\/;

function isAbsolutePath(path: string): boolean {
  return path.startsWith("/") || WINDOWS_ABSOLUTE_PATH_REGEX.test(path) || WINDOWS_UNC_PATH_REGEX.test(path);
}

export default function App() {
  const bootstrap = useMemo(() => resolveBootstrap(window), []);
  const [state, setState] = useState<ConnectionState>("idle");
  const [client, setClient] = useState<HarmonyWebSocketClient>();
  const hostBridgeClientRef = useRef<HarmonyHostBridgeClient | null>(null);
  const [canBrowseHdcPath, setCanBrowseHdcPath] = useState(false);
  const [canOpenInEditor, setCanOpenInEditor] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [activeMainTab, setActiveMainTab] = useState<MainPanelTabId>(() =>
    readPersistedMainTab(bootstrap.host, DEFAULT_MAIN_PANEL_TAB_ID)
  );
  const [appSettings, setAppSettings] = useState<AppSettings>(() => readAppSettings(bootstrap.host));

  useEffect(() => {
    const hostBridge = createHostBridgeClient(window);
    hostBridgeClientRef.current = hostBridge;
    setCanBrowseHdcPath(false);
    setCanOpenInEditor(false);

    let cancelled = false;
    void Promise.all([
      hostBridge.getCapabilities().catch(() => null),
      hostBridge.getHostInfo().catch(() => null)
    ])
      .then(([capabilities, hostInfo]) => {
        if (cancelled) {
          return;
        }

        setCanBrowseHdcPath(Boolean(capabilities?.["ide.openFilePicker"]));
        const isVsCodeApiHost =
          hostInfo?.host === "vscode" || hostInfo?.host === "cursor" || hostInfo?.host === "trae";
        setCanOpenInEditor(Boolean(capabilities?.["ide.openFile"]) && isVsCodeApiHost);
      })
      .catch(() => {
        if (cancelled) {
          return;
        }

        setCanBrowseHdcPath(false);
        setCanOpenInEditor(false);
      });

    return () => {
      cancelled = true;
      hostBridge.dispose();
      if (hostBridgeClientRef.current === hostBridge) {
        hostBridgeClientRef.current = null;
      }
    };
  }, [bootstrap.host]);

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

  const browseHdcPath = useCallback(async (): Promise<string | null> => {
    const hostBridgeClient = hostBridgeClientRef.current;
    if (!hostBridgeClient) {
      return null;
    }

    const defaultPathCandidate = hdcBinConfig.customBinPath ?? hdcBinConfig.resolvedBinPath ?? undefined;
    const defaultPath =
      defaultPathCandidate && isAbsolutePath(defaultPathCandidate) ? defaultPathCandidate : undefined;

    const result = await hostBridgeClient.invoke("ide.openFilePicker", {
      canSelectFiles: true,
      canSelectFolders: false,
      canSelectMany: false,
      title: "Select HDC binary",
      defaultPath
    });

    if (result.canceled || result.paths.length === 0) {
      return null;
    }

    return result.paths[0] ?? null;
  }, [hdcBinConfig.customBinPath, hdcBinConfig.resolvedBinPath]);

  const pickUploadFiles = useCallback(
    async (_targetDirectoryPath: string): Promise<readonly string[] | null> => {
      const hostBridgeClient = hostBridgeClientRef.current;
      if (!hostBridgeClient) {
        return null;
      }

      const result = await hostBridgeClient.invoke("ide.openFilePicker", {
        canSelectFiles: true,
        canSelectFolders: false,
        canSelectMany: true,
        title: "Select files to upload"
      });

      if (result.canceled || result.paths.length === 0) {
        return null;
      }

      return result.paths;
    },
    []
  );

  const pickDownloadDirectory = useCallback(
    async (_sourceFilePath: string): Promise<string | null> => {
      const hostBridgeClient = hostBridgeClientRef.current;
      if (!hostBridgeClient) {
        return null;
      }

      const result = await hostBridgeClient.invoke("ide.openFilePicker", {
        canSelectFiles: false,
        canSelectFolders: true,
        canSelectMany: false,
        title: "Select download folder"
      });

      if (result.canceled || result.paths.length === 0) {
        return null;
      }

      return result.paths[0] ?? null;
    },
    []
  );

  const openLocalPathInEditor = useCallback(async (localPath: string): Promise<void> => {
    const hostBridgeClient = hostBridgeClientRef.current;
    if (!hostBridgeClient) {
      throw new Error("IDE host bridge is not available.");
    }

    await hostBridgeClient.invoke("ide.openFile", {
      path: localPath,
      preview: false,
      preserveFocus: false
    });
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
        hostFilePickerAvailable={canBrowseHdcPath}
        openInEditorAvailable={canOpenInEditor}
        openLocalPathInEditor={openLocalPathInEditor}
        pickUploadFiles={pickUploadFiles}
        pickDownloadDirectory={pickDownloadDirectory}
      />
    ),
    mcpTools: (
      <McpToolsPanel
        client={client}
        connectionState={state}
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
        canBrowseHdcPath={canBrowseHdcPath}
        onBrowseHdcPath={browseHdcPath}
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
