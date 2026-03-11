import type {
  EmulatorCreateDeviceOptionsResult,
  EmulatorDeviceSummary,
  EmulatorDownloadFailedEventData,
  EmulatorDownloadFinishedEventData,
  EmulatorDownloadJobSummary,
  EmulatorDownloadProgressEventData,
  EmulatorEnvironmentResult,
  EmulatorImageSummary,
  HostCapabilities,
  HostMessage
} from "@harmony/protocol";
import type { ConnectionState, HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { useCallback, useEffect, useMemo, useState } from "react";

type LoadStatus = "idle" | "loading" | "ready" | "unsupported" | "error";
type NoticeTone = "success" | "error" | "warning";
type InstallationFilter = "uninstalled" | "installed";
type DeviceAction = "start" | "stop" | "delete";
type CreateDialogState = {
  image: EmulatorImageSummary;
  options: EmulatorCreateDeviceOptionsResult;
  productDeviceType: string;
  productName: string;
  name: string;
  cpuCores: string;
  memoryRamMb: string;
  dataDiskMb: string;
  vendorCountry: string;
  isPublic: boolean;
  loading: boolean;
  submitting: boolean;
  errorMessage?: string;
};

const REQUIRED_CAPABILITIES: ReadonlyArray<keyof HostCapabilities> = [
  "emulator.getEnvironment",
  "emulator.listImages",
  "emulator.listDownloadJobs",
  "emulator.getCreateDeviceOptions",
  "emulator.downloadImage",
  "emulator.listDevices",
  "emulator.createDevice",
  "emulator.startDevice",
  "emulator.stopDevice",
  "emulator.deleteDevice"
];
const AUTO_SELECT_TIMEOUT_MS = 30_000;
const AUTO_SELECT_POLL_MS = 1_500;

interface EmulatorsPanelProps {
  client?: HarmonyWebSocketClient;
  connectionState: ConnectionState;
  hdcAvailable: boolean;
  currentHdcDevices: readonly string[];
  selectHdcDevice: (connectKey: string) => void;
}

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function formatBytes(value?: number | null): string {
  if (!value || value <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB"];
  let size = value;
  let unitIndex = 0;

  while (size >= 1024 && unitIndex < units.length - 1) {
    size /= 1024;
    unitIndex += 1;
  }

  return `${size >= 10 || unitIndex === 0 ? size.toFixed(0) : size.toFixed(1)} ${units[unitIndex]}`;
}

function buildDefaultDeviceName(productName: string): string {
  const normalized = productName.trim().replace(/\s+/g, "_").replace(/[^A-Za-z0-9_]/g, "");
  return normalized ? `${normalized}_Emulator` : "Harmony_Emulator";
}

function upsertJob(
  jobs: readonly EmulatorDownloadJobSummary[],
  nextJob: EmulatorDownloadJobSummary
): EmulatorDownloadJobSummary[] {
  const nextJobs = jobs.filter((job) => job.jobId !== nextJob.jobId);
  nextJobs.unshift(nextJob);
  return nextJobs;
}

function isProgressEvent(
  message: HostMessage
): message is HostMessage & {
  type: "event";
  payload: { name: "emulator.download.progress"; data: EmulatorDownloadProgressEventData };
} {
  return message.type === "event" && message.payload.name === "emulator.download.progress";
}

function isFinishedEvent(
  message: HostMessage
): message is HostMessage & {
  type: "event";
  payload: { name: "emulator.download.finished"; data: EmulatorDownloadFinishedEventData };
} {
  return message.type === "event" && message.payload.name === "emulator.download.finished";
}

function isFailedEvent(
  message: HostMessage
): message is HostMessage & {
  type: "event";
  payload: { name: "emulator.download.failed"; data: EmulatorDownloadFailedEventData };
} {
  return message.type === "event" && message.payload.name === "emulator.download.failed";
}

function Placeholder({
  message,
  errorMessage
}: {
  message: string;
  errorMessage?: string;
}) {
  return (
    <section className="panel emulators-panel" aria-label="Emulators">
      <p className="kicker">Emulators</p>
      <h2>Emulators</h2>
      <p className="panel-message">{message}</p>
      {errorMessage ? <p className="panel-message panel-message-error">{errorMessage}</p> : null}
    </section>
  );
}

export function EmulatorsPanel({
  client,
  connectionState,
  hdcAvailable,
  currentHdcDevices,
  selectHdcDevice
}: EmulatorsPanelProps) {
  const [status, setStatus] = useState<LoadStatus>("idle");
  const [environment, setEnvironment] = useState<EmulatorEnvironmentResult>();
  const [images, setImages] = useState<EmulatorImageSummary[]>([]);
  const [devices, setDevices] = useState<EmulatorDeviceSummary[]>([]);
  const [jobs, setJobs] = useState<EmulatorDownloadJobSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [installationFilter, setInstallationFilter] = useState<InstallationFilter>("uninstalled");
  const [apiLevelFilter, setApiLevelFilter] = useState("all");
  const [deviceTypeFilter, setDeviceTypeFilter] = useState("all");
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string>();
  const [notice, setNotice] = useState<{ tone: NoticeTone; text: string } | null>(null);
  const [pendingImagePath, setPendingImagePath] = useState<string | null>(null);
  const [pendingDeviceAction, setPendingDeviceAction] = useState<{
    name: string;
    action: DeviceAction;
  } | null>(null);
  const [createDialog, setCreateDialog] = useState<CreateDialogState | null>(null);

  const refreshImages = useCallback(async () => {
    if (!client || connectionState !== "open") {
      return;
    }

    const result = await client.invoke("emulator.listImages", {});
    setImages(result.images);
  }, [client, connectionState]);

  const refreshDevices = useCallback(async () => {
    if (!client || connectionState !== "open") {
      return;
    }

    const result = await client.invoke("emulator.listDevices", {});
    setDevices(result.devices);
  }, [client, connectionState]);

  const refreshAll = useCallback(async () => {
    if (!client || connectionState !== "open") {
      return;
    }

    setIsRefreshing(true);
    setErrorMessage(undefined);

    try {
      const [environmentResult, imageResult, deviceResult, jobResult] = await Promise.all([
        client.invoke("emulator.getEnvironment", {}),
        client.invoke("emulator.listImages", {}),
        client.invoke("emulator.listDevices", {}),
        client.invoke("emulator.listDownloadJobs", {})
      ]);

      setEnvironment(environmentResult);
      setImages(imageResult.images);
      setDevices(deviceResult.devices);
      setJobs(jobResult.jobs);
      setStatus("ready");
    } catch (error) {
      setStatus("error");
      setErrorMessage(toErrorMessage(error));
    } finally {
      setIsRefreshing(false);
    }
  }, [client, connectionState]);

  useEffect(() => {
    let cancelled = false;

    const initialize = async () => {
      if (!client || connectionState !== "open") {
        setStatus("idle");
        setEnvironment(undefined);
        setImages([]);
        setDevices([]);
        setJobs([]);
        setErrorMessage(undefined);
        return;
      }

      setStatus("loading");
      setErrorMessage(undefined);

      try {
        const capabilityResult = await client.invoke("host.getCapabilities", {});
        if (cancelled) {
          return;
        }

        const supported = REQUIRED_CAPABILITIES.every((capability) => capabilityResult.capabilities[capability]);
        if (!supported) {
          setStatus("unsupported");
          return;
        }

        await refreshAll();
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
  }, [client, connectionState, refreshAll]);

  useEffect(() => {
    if (!client) {
      return;
    }

    return client.onMessage((message) => {
      if (isProgressEvent(message)) {
        setJobs((currentJobs) =>
          upsertJob(currentJobs, {
            jobId: message.payload.data.jobId,
            imageRelativePath: message.payload.data.imageRelativePath,
            stage: message.payload.data.stage,
            status: message.payload.data.status,
            progress: message.payload.data.progress,
            increment: message.payload.data.increment,
            network: message.payload.data.network ?? undefined,
            unit: message.payload.data.unit ?? undefined,
            reset: message.payload.data.reset
          })
        );
        setImages((currentImages) =>
          currentImages.map((image) =>
            image.relativePath === message.payload.data.imageRelativePath
              ? { ...image, status: "downloading" }
              : image
          )
        );
        return;
      }

      if (isFinishedEvent(message)) {
        setJobs((currentJobs) =>
          upsertJob(currentJobs, {
            jobId: message.payload.data.jobId,
            imageRelativePath: message.payload.data.imageRelativePath,
            stage: message.payload.data.stage,
            status: message.payload.data.status,
            progress: 100,
            increment: 0,
            reset: false
          })
        );
        setNotice({
          tone: "success",
          text: `Downloaded ${message.payload.data.image.displayName}.`
        });
        void refreshImages();
        return;
      }

      if (isFailedEvent(message)) {
        setJobs((currentJobs) =>
          upsertJob(currentJobs, {
            jobId: message.payload.data.jobId,
            imageRelativePath: message.payload.data.imageRelativePath,
            stage: message.payload.data.stage,
            status: message.payload.data.status,
            progress: 0,
            increment: 0,
            reset: false,
            message: message.payload.data.message
          })
        );
        setNotice({
          tone: "error",
          text: message.payload.data.message
        });
        void refreshImages();
      }
    });
  }, [client, refreshImages]);

  const activeJobByImagePath = useMemo(() => {
    const map = new Map<string, EmulatorDownloadJobSummary>();
    for (const job of jobs) {
      if (!map.has(job.imageRelativePath) || job.status === "running") {
        map.set(job.imageRelativePath, job);
      }
    }
    return map;
  }, [jobs]);

  const filteredImages = useMemo(() => {
    const normalizedQuery = searchQuery.trim().toLowerCase();

    return images.filter((image) => {
      if (
        (installationFilter === "installed" && image.status !== "installed") ||
        (installationFilter === "uninstalled" && image.status === "installed")
      ) {
        return false;
      }

      if (apiLevelFilter !== "all" && String(image.apiVersion) !== apiLevelFilter) {
        return false;
      }

      if (deviceTypeFilter !== "all" && image.deviceType !== deviceTypeFilter) {
        return false;
      }

      if (!normalizedQuery) {
        return true;
      }

      return (
        image.displayName.toLowerCase().includes(normalizedQuery) ||
        image.relativePath.toLowerCase().includes(normalizedQuery) ||
        image.deviceType.toLowerCase().includes(normalizedQuery)
      );
    });
  }, [apiLevelFilter, deviceTypeFilter, images, installationFilter, searchQuery]);

  const deviceTypes = useMemo(
    () => Array.from(new Set(images.map((image) => image.deviceType))).sort(),
    [images]
  );

  const apiLevels = useMemo(
    () => Array.from(new Set(images.map((image) => image.apiVersion))).sort((left, right) => left - right),
    [images]
  );

  const openCreateDialog = useCallback(async (image: EmulatorImageSummary) => {
    if (!client || connectionState !== "open") {
      return;
    }

    if (image.status !== "installed") {
      setNotice({
        tone: "warning",
        text: "Download and extract the image before creating an emulator device."
      });
      return;
    }

    setPendingImagePath(image.relativePath);
    setCreateDialog({
      image,
      options: {
        imageRelativePath: image.relativePath,
        productPresets: []
      },
      productDeviceType: "",
      productName: "",
      name: "",
      cpuCores: "",
      memoryRamMb: "",
      dataDiskMb: "",
      vendorCountry: "CN",
      isPublic: true,
      loading: true,
      submitting: false
    });

    try {
      const options = await client.invoke("emulator.getCreateDeviceOptions", {
        relativePath: image.relativePath
      });
      const firstPreset = options.productPresets[0];

      if (!firstPreset) {
        setCreateDialog((current) =>
          current
            ? {
                ...current,
                options,
                loading: false,
                errorMessage: "No catalog-backed presets are available for this image."
              }
            : current
        );
        return;
      }

      setCreateDialog({
        image,
        options,
        productDeviceType: firstPreset.deviceType,
        productName: firstPreset.name,
        name: buildDefaultDeviceName(firstPreset.name),
        cpuCores: String(firstPreset.defaultCpuCores),
        memoryRamMb: String(firstPreset.defaultMemoryRamMb),
        dataDiskMb: String(firstPreset.defaultDataDiskMb),
        vendorCountry: "CN",
        isPublic: true,
        loading: false,
        submitting: false
      });
    } catch (error) {
      setCreateDialog((current) =>
        current
          ? {
              ...current,
              loading: false,
              errorMessage: toErrorMessage(error)
            }
          : current
      );
    } finally {
      setPendingImagePath(null);
    }
  }, [client, connectionState]);

  const handleDownload = useCallback(
    async (image: EmulatorImageSummary) => {
      if (!client || connectionState !== "open") {
        return;
      }

      setPendingImagePath(image.relativePath);
      setNotice(null);

      try {
        const result = await client.invoke("emulator.downloadImage", {
          relativePath: image.relativePath
        });

        setJobs((currentJobs) =>
          upsertJob(currentJobs, {
            jobId: result.jobId,
            imageRelativePath: image.relativePath,
            stage: "download",
            status: "running",
            progress: 0,
            increment: 0,
            reset: false
          })
        );
        await refreshImages();
        setNotice({
          tone: "success",
          text: `Started download for ${image.displayName}.`
        });
      } catch (error) {
        setNotice({
          tone: "error",
          text: toErrorMessage(error)
        });
      } finally {
        setPendingImagePath(null);
      }
    },
    [client, connectionState, refreshImages]
  );

  const handleCreateSubmit = useCallback(async () => {
    if (!createDialog) {
      return;
    }

    if (!client || connectionState !== "open") {
      const message = "Host connection is not open. Reconnect the bridge and try again.";
      setCreateDialog((current) =>
        current
          ? {
              ...current,
              submitting: false,
              errorMessage: message
            }
          : current
      );
      setNotice({
        tone: "error",
        text: message
      });
      return;
    }

    if (createDialog.loading || !createDialog.productDeviceType || !createDialog.productName) {
      const message = "No catalog-backed preset is ready for this image yet.";
      setCreateDialog((current) =>
        current
          ? {
              ...current,
              submitting: false,
              errorMessage: message
            }
          : current
      );
      setNotice({
        tone: "error",
        text: message
      });
      return;
    }

    const cpuCores = Number.parseInt(createDialog.cpuCores, 10);
    const memoryRamMb = Number.parseInt(createDialog.memoryRamMb, 10);
    const dataDiskMb = Number.parseInt(createDialog.dataDiskMb, 10);
    if (
      !createDialog.name.trim() ||
      !Number.isFinite(cpuCores) ||
      cpuCores <= 0 ||
      !Number.isFinite(memoryRamMb) ||
      memoryRamMb <= 0 ||
      !Number.isFinite(dataDiskMb) ||
      dataDiskMb <= 0
    ) {
      setCreateDialog((current) =>
        current
          ? {
              ...current,
              errorMessage: "Name, CPU, RAM, and data disk size must be valid positive values."
            }
          : current
      );
      setNotice({
        tone: "error",
        text: "Name, CPU, RAM, and data disk size must be valid positive values."
      });
      return;
    }

    setCreateDialog((current) =>
      current
        ? {
            ...current,
            submitting: true,
            errorMessage: undefined
          }
        : current
    );
    setNotice(null);

    try {
      const result = await client.invoke("emulator.createDevice", {
        relativePath: createDialog.image.relativePath,
        productDeviceType: createDialog.productDeviceType,
        productName: createDialog.productName,
        name: createDialog.name.trim(),
        cpuCores,
        memoryRamMb,
        dataDiskMb,
        vendorCountry: createDialog.vendorCountry.trim() || undefined,
        isPublic: createDialog.isPublic
      });
      setDevices((current) => {
        const nextDevices = current.filter((device) => device.name !== result.device.name);
        nextDevices.unshift(result.device);
        return nextDevices;
      });
      setCreateDialog(null);
      setNotice({
        tone: "success",
        text: `Created emulator device ${createDialog.name.trim()}.`
      });
      void refreshDevices();
    } catch (error) {
      const message = toErrorMessage(error);
      setCreateDialog((current) =>
        current
          ? {
              ...current,
              submitting: false,
              errorMessage: message
            }
          : current
      );
      setNotice({
        tone: "error",
        text: message
      });
    }
  }, [client, connectionState, createDialog, refreshDevices]);

  const autoSelectStartedDevice = useCallback(
    async (beforeTargets: readonly string[]) => {
      if (!client || connectionState !== "open" || !hdcAvailable) {
        return;
      }

      const deadline = Date.now() + AUTO_SELECT_TIMEOUT_MS;
      const knownTargets = new Set(beforeTargets);

      while (Date.now() < deadline) {
        await new Promise((resolve) => {
          window.setTimeout(resolve, AUTO_SELECT_POLL_MS);
        });

        try {
          const result = await client.invoke("hdc.listTargets", {});
          const newTargets = result.targets.filter((target) => !knownTargets.has(target));

          if (newTargets.length === 1) {
            selectHdcDevice(newTargets[0]);
            setNotice({
              tone: "success",
              text: `Auto-selected HDC target ${newTargets[0]}.`
            });
            return;
          }

          if (newTargets.length > 1) {
            setNotice({
              tone: "warning",
              text: "Emulator started, but multiple new HDC targets appeared. Select one manually."
            });
            return;
          }
        } catch {
          break;
        }
      }

      setNotice({
        tone: "warning",
        text: "Emulator started, but HDC auto-select timed out."
      });
    },
    [client, connectionState, hdcAvailable, selectHdcDevice]
  );

  const handleDeviceAction = useCallback(
    async (deviceName: string, action: DeviceAction) => {
      if (!client || connectionState !== "open") {
        return;
      }

      setPendingDeviceAction({ name: deviceName, action });
      setNotice(null);

      const beforeTargets = [...currentHdcDevices];

      try {
        if (action === "start") {
          await client.invoke("emulator.startDevice", { name: deviceName });
          setNotice({
            tone: "success",
            text: `Launch command sent for ${deviceName}.`
          });
          void autoSelectStartedDevice(beforeTargets);
        } else if (action === "stop") {
          await client.invoke("emulator.stopDevice", { name: deviceName });
          setNotice({
            tone: "success",
            text: `Stop command sent for ${deviceName}.`
          });
        } else {
          await client.invoke("emulator.deleteDevice", { name: deviceName });
          setNotice({
            tone: "success",
            text: `Deleted emulator device ${deviceName}.`
          });
        }

        await refreshDevices();
      } catch (error) {
        setNotice({
          tone: "error",
          text: toErrorMessage(error)
        });
      } finally {
        setPendingDeviceAction(null);
      }
    },
    [autoSelectStartedDevice, client, connectionState, currentHdcDevices, refreshDevices]
  );

  if (connectionState !== "open") {
    return <Placeholder message="Waiting for websocket connection." />;
  }

  if (status === "loading" || status === "idle") {
    return <Placeholder message="Loading emulator capabilities..." />;
  }

  if (status === "unsupported") {
    return (
      <Placeholder message="This bridge version does not expose emulator-management APIs yet." />
    );
  }

  if (status === "error") {
    return (
      <Placeholder
        message="Failed to load emulator-management data."
        errorMessage={errorMessage}
      />
    );
  }

  return (
    <section className="panel emulators-panel" aria-label="Emulators">
      <div className="emulators-toolbar">
        <div className="emulators-toolbar-left">
          {isRefreshing ? <span className="panel-message">Refreshing...</span> : null}
          {environment?.message ? (
            <p className="panel-message panel-message-warning">{environment.message}</p>
          ) : null}
        </div>

        <button
          type="button"
          className="device-refresh"
          disabled={isRefreshing}
          onClick={() => {
            void refreshAll();
          }}
        >
          {isRefreshing ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      {notice ? (
        <p
          className={`panel-message ${
            notice.tone === "error"
              ? "panel-message-error"
              : notice.tone === "warning"
                ? "panel-message-warning"
                : ""
          }`}
        >
          {notice.text}
        </p>
      ) : null}

      <div className="emulators-content-grid">
        <section className="emulators-images">
          <div className="emulators-section-header">
            <h2>Images</h2>
          </div>

          <div className="emulators-filter-row">
            <select
              className="emulators-filter-select"
              value={apiLevelFilter}
              onChange={(event) => {
                setApiLevelFilter(event.target.value);
              }}
            >
              <option value="all">All API levels</option>
              {apiLevels.map((apiLevel) => (
                <option key={apiLevel} value={String(apiLevel)}>
                  API {apiLevel}
                </option>
              ))}
            </select>
            <select
              className="emulators-filter-select"
              value={deviceTypeFilter}
              onChange={(event) => {
                setDeviceTypeFilter(event.target.value);
              }}
            >
              <option value="all">All device types</option>
              {deviceTypes.map((deviceType) => (
                <option key={deviceType} value={deviceType}>
                  {deviceType}
                </option>
              ))}
            </select>
            <input
              type="search"
              className="emulators-search-input"
              value={searchQuery}
              placeholder="Search images"
              onChange={(event) => {
                setSearchQuery(event.target.value);
              }}
            />
          </div>

          <div
            className="main-tabs emulators-list-tabs"
            role="tablist"
            aria-label="Image installation status"
          >
            <button
              type="button"
              role="tab"
              aria-selected={installationFilter === "uninstalled"}
              className="main-tab-button emulators-list-tab"
              onClick={() => {
                setInstallationFilter("uninstalled");
              }}
            >
              Uninstalled
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={installationFilter === "installed"}
              className="main-tab-button emulators-list-tab"
              onClick={() => {
                setInstallationFilter("installed");
              }}
            >
              Installed
            </button>
          </div>

          <div className="emulators-image-list">
            {filteredImages.length === 0 ? (
              <p className="panel-message">No images match the current filters.</p>
            ) : (
              filteredImages.map((image) => {
                const activeJob = activeJobByImagePath.get(image.relativePath);
                const isPending = pendingImagePath === image.relativePath;
                const isDownloading =
                  image.status === "downloading" && activeJob?.status === "running";

                return (
                  <article key={image.relativePath} className="emulators-image-row">
                    <div className="emulators-image-row-main">
                      <div className="emulators-image-row-header">
                        <div className="emulators-image-row-title">
                          <strong>{image.displayName}</strong>
                          {image.status !== "available" ? (
                            <span className="emulators-status-pill">{image.status}</span>
                          ) : null}
                        </div>
                        <div className="emulators-image-row-actions">
                          {image.status === "installed" ? (
                            <button
                              type="button"
                              className="device-refresh"
                              disabled={isPending}
                              onClick={() => {
                                void openCreateDialog(image);
                              }}
                            >
                              {isPending ? "Loading..." : "Create device"}
                            </button>
                          ) : (
                            <button
                              type="button"
                              className="device-refresh"
                              disabled={isPending || isDownloading}
                              onClick={() => {
                                void handleDownload(image);
                              }}
                            >
                              {isPending
                                ? "Starting..."
                                : isDownloading
                                  ? "Downloading..."
                                  : "Download"}
                            </button>
                          )}
                        </div>
                      </div>
                      <div className="emulators-image-meta">
                        <span className="panel-message">
                          API {image.apiVersion} · {image.deviceType} · {image.version}
                        </span>
                        <span className="panel-message">
                          {image.releaseType || "Unknown"} · {formatBytes(image.archiveSizeBytes)}
                        </span>
                        <span className="panel-message">
                          {image.localPath ?? image.relativePath}
                        </span>
                      </div>
                      {image.description ? (
                        <p className="panel-message emulators-image-description">{image.description}</p>
                      ) : null}

                      {activeJob ? (
                        <div className="emulators-download-card emulators-download-card-inline">
                          <div className="emulators-download-header">
                            <strong>{activeJob.stage}</strong>
                            <span>
                              {activeJob.status === "running"
                                ? `${activeJob.progress.toFixed(0)}%`
                                : activeJob.status}
                            </span>
                          </div>
                          <div className="emulators-progress-bar" aria-hidden="true">
                            <span
                              className="emulators-progress-bar-fill"
                              style={{
                                width: `${Math.max(0, Math.min(100, activeJob.progress))}%`
                              }}
                            />
                          </div>
                          <p className="panel-message">
                            {activeJob.network && activeJob.unit
                              ? `${activeJob.network.toFixed(1)} ${activeJob.unit}/s`
                              : activeJob.message ?? "Waiting for the next progress update."}
                          </p>
                        </div>
                      ) : null}
                    </div>
                  </article>
                );
              })
            )}
          </div>
        </section>

        <section className="emulators-devices">
          <div className="emulators-section-header">
            <h2>Instances</h2>
          </div>

          {devices.length === 0 ? (
            <p className="panel-message">No emulator devices have been created yet.</p>
          ) : (
            <div className="emulators-device-grid">
              {devices.map((device) => {
                const isPending = pendingDeviceAction?.name === device.name;

                return (
                  <article key={device.name} className="emulators-device-card">
                    <div className="emulators-device-meta">
                      <strong>{device.name}</strong>
                      <span className="panel-message">
                        {device.model ?? device.deviceType} · API {device.apiVersion}
                      </span>
                      <span className="panel-message">{device.showVersion}</span>
                      <span className="panel-message">
                        {formatBytes(device.storageSizeBytes)} · {device.instancePath}
                      </span>
                    </div>
                    <div className="emulators-device-actions">
                      <button
                        type="button"
                        className="device-refresh"
                        disabled={Boolean(isPending)}
                        onClick={() => {
                          void handleDeviceAction(device.name, "start");
                        }}
                      >
                        {isPending && pendingDeviceAction?.action === "start" ? "Starting..." : "Start"}
                      </button>
                      <button
                        type="button"
                        className="emulators-secondary-button"
                        disabled={Boolean(isPending)}
                        onClick={() => {
                          void handleDeviceAction(device.name, "stop");
                        }}
                      >
                        {isPending && pendingDeviceAction?.action === "stop" ? "Stopping..." : "Stop"}
                      </button>
                      <button
                        type="button"
                        className="emulators-danger-button"
                        disabled={Boolean(isPending)}
                        onClick={() => {
                          void handleDeviceAction(device.name, "delete");
                        }}
                      >
                        {isPending && pendingDeviceAction?.action === "delete" ? "Deleting..." : "Delete"}
                      </button>
                    </div>
                  </article>
                );
              })}
            </div>
          )}
        </section>
      </div>

      {createDialog ? (
        <div
          className="emulators-dialog-backdrop"
          role="presentation"
          onClick={() => {
            if (!createDialog.submitting) {
              setCreateDialog(null);
            }
          }}
        >
          <div
            className="emulators-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="emulators-create-title"
            onClick={(event) => {
              event.stopPropagation();
            }}
          >
            <div className="emulators-dialog-header">
              <div>
                <p className="kicker">Create Device</p>
                <h2 id="emulators-create-title">{createDialog.image.displayName}</h2>
              </div>
              <button
                type="button"
                className="settings-icon-button"
                aria-label="Close create device dialog"
                onClick={() => {
                  setCreateDialog(null);
                }}
                disabled={createDialog.submitting}
              >
                X
              </button>
            </div>

            {createDialog.loading ? (
              <p className="panel-message">Loading catalog-backed presets...</p>
            ) : (
              <div className="emulators-dialog-form">
                <label className="emulators-field">
                  <span>Preset</span>
                  <select
                    value={`${createDialog.productDeviceType}::${createDialog.productName}`}
                    disabled={createDialog.submitting}
                    onChange={(event) => {
                      const [nextDeviceType, nextProductName] = event.target.value.split("::");
                      const preset = createDialog.options.productPresets.find(
                        (entry) => entry.deviceType === nextDeviceType && entry.name === nextProductName
                      );
                      if (!preset) {
                        return;
                      }

                      setCreateDialog((current) =>
                        current
                          ? {
                              ...current,
                              productDeviceType: preset.deviceType,
                              productName: preset.name,
                              name: buildDefaultDeviceName(preset.name),
                              cpuCores: String(preset.defaultCpuCores),
                              memoryRamMb: String(preset.defaultMemoryRamMb),
                              dataDiskMb: String(preset.defaultDataDiskMb)
                            }
                          : current
                      );
                    }}
                  >
                    {createDialog.options.productPresets.map((preset) => (
                      <option
                        key={`${preset.deviceType}:${preset.name}`}
                        value={`${preset.deviceType}::${preset.name}`}
                      >
                        {preset.deviceType} · {preset.name}
                      </option>
                    ))}
                  </select>
                </label>

                <label className="emulators-field">
                  <span>Name</span>
                  <input
                    type="text"
                    value={createDialog.name}
                    disabled={createDialog.submitting}
                    onChange={(event) => {
                      setCreateDialog((current) =>
                        current
                          ? {
                              ...current,
                              name: event.target.value
                            }
                          : current
                      );
                    }}
                  />
                </label>

                <div className="emulators-field-row">
                  <label className="emulators-field">
                    <span>CPU cores</span>
                    <input
                      type="number"
                      min={1}
                      value={createDialog.cpuCores}
                      disabled={createDialog.submitting}
                      onChange={(event) => {
                        setCreateDialog((current) =>
                          current
                            ? {
                                ...current,
                                cpuCores: event.target.value
                              }
                            : current
                        );
                      }}
                    />
                  </label>
                  <label className="emulators-field">
                    <span>RAM (MB)</span>
                    <input
                      type="number"
                      min={1}
                      value={createDialog.memoryRamMb}
                      disabled={createDialog.submitting}
                      onChange={(event) => {
                        setCreateDialog((current) =>
                          current
                            ? {
                                ...current,
                                memoryRamMb: event.target.value
                              }
                            : current
                        );
                      }}
                    />
                  </label>
                  <label className="emulators-field">
                    <span>Data disk (MB)</span>
                    <input
                      type="number"
                      min={1}
                      value={createDialog.dataDiskMb}
                      disabled={createDialog.submitting}
                      onChange={(event) => {
                        setCreateDialog((current) =>
                          current
                            ? {
                                ...current,
                                dataDiskMb: event.target.value
                              }
                            : current
                        );
                      }}
                    />
                  </label>
                </div>

                <div className="emulators-field-row">
                  <label className="emulators-field">
                    <span>Vendor country</span>
                    <input
                      type="text"
                      maxLength={4}
                      value={createDialog.vendorCountry}
                      disabled={createDialog.submitting}
                      onChange={(event) => {
                        setCreateDialog((current) =>
                          current
                            ? {
                                ...current,
                                vendorCountry: event.target.value.toUpperCase()
                              }
                            : current
                        );
                      }}
                    />
                  </label>
                  <label className="emulators-checkbox">
                    <input
                      type="checkbox"
                      checked={createDialog.isPublic}
                      disabled={createDialog.submitting}
                      onChange={(event) => {
                        setCreateDialog((current) =>
                          current
                            ? {
                                ...current,
                                isPublic: event.target.checked
                              }
                            : current
                        );
                      }}
                    />
                    <span>Public image</span>
                  </label>
                </div>

                {createDialog.errorMessage ? (
                  <p className="panel-message panel-message-error">{createDialog.errorMessage}</p>
                ) : null}

                <div className="emulators-dialog-actions">
                  <button
                    type="button"
                    className="emulators-secondary-button"
                    disabled={createDialog.submitting}
                    onClick={() => {
                      setCreateDialog(null);
                    }}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="device-refresh"
                    disabled={createDialog.submitting}
                    onClick={() => {
                      void handleCreateSubmit();
                    }}
                  >
                    {createDialog.submitting ? "Creating..." : "Create device"}
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      ) : null}
    </section>
  );
}
