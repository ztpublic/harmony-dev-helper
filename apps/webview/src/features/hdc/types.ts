import type { HostCapabilities } from "@harmony/protocol";

export type DeviceLoadState = "idle" | "loading" | "ready" | "unsupported" | "error";

export interface DeviceSelectionState {
  capabilities: HostCapabilities | null;
  isSupported: boolean;
  status: DeviceLoadState;
  devices: string[];
  selectedDevice: string | null;
  isRefreshing: boolean;
  errorMessage?: string;
}
