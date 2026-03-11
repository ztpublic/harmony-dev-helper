import type { Meta, StoryObj } from "@storybook/react";
import type {
  EmulatorCreateDeviceOptionsResult,
  EmulatorDeviceSummary,
  EmulatorDownloadJobSummary,
  EmulatorEnvironmentResult,
  EmulatorImageSummary,
  HostCapabilities,
  InvokeAction
} from "@harmony/protocol";
import type { HarmonyWebSocketClient } from "@harmony/webview-bridge";
import { EmulatorsPanel } from "./EmulatorsPanel";

const capabilities: HostCapabilities = {
  "host.getCapabilities": true,
  "mcp.listTools": true,
  "hdc.listTargets": true,
  "hdc.getParameters": true,
  "hdc.shell": true,
  "hdc.fs.list": true,
  "hdc.fs.upload": true,
  "hdc.fs.download": true,
  "hdc.fs.downloadTemp": true,
  "hdc.fs.delete": true,
  "hdc.getBinConfig": true,
  "hdc.setBinPath": true,
  "hdc.hilog.listPids": true,
  "hdc.hilog.subscribe": true,
  "hdc.hilog.unsubscribe": true,
  "emulator.getEnvironment": true,
  "emulator.listImages": true,
  "emulator.listDownloadJobs": true,
  "emulator.getCreateDeviceOptions": true,
  "emulator.downloadImage": true,
  "emulator.listDevices": true,
  "emulator.createDevice": true,
  "emulator.startDevice": true,
  "emulator.stopDevice": true,
  "emulator.deleteDevice": true
};

const environment: EmulatorEnvironmentResult = {
  compatibility: true,
  paths: {
    imageBasePath: "/Users/demo/Library/OpenHarmony/images",
    deployedPath: "/Users/demo/Library/OpenHarmony/deployed",
    cachePath: "/Users/demo/Library/OpenHarmony/cache",
    sdkPath: "/Users/demo/Library/OpenHarmony/sdk/default/openharmony",
    configPath: "/Users/demo/Library/OpenHarmony/config",
    logPath: "/Users/demo/Library/OpenHarmony/log",
    emulatorPath: "/Applications/DevEco-Studio.app/Contents/tools/emulator"
  }
};

const images: EmulatorImageSummary[] = [
  {
    relativePath: "system-image/HarmonyOS-6.0.1/phone_all",
    displayName: "System-image-phone_all",
    apiVersion: 21,
    deviceType: "phone_all",
    version: "6.0.0.112",
    platformVersion: "6.0.1",
    guestVersion: "HarmonyOS 6.0.1",
    releaseType: "Release",
    description: "Shared phone-family emulator image.",
    status: "installed",
    localPath: "/Users/demo/Library/OpenHarmony/images/system-image/HarmonyOS-6.0.1/phone_all",
    archiveSizeBytes: 734003200,
    checksum: "abc123"
  },
  {
    relativePath: "system-image/HarmonyOS-6.0.1/pc_all",
    displayName: "System-image-pc_all",
    apiVersion: 21,
    deviceType: "pc_all",
    version: "6.0.0.112",
    platformVersion: "6.0.1",
    guestVersion: "HarmonyOS 6.0.1",
    releaseType: "Release",
    description: "Shared PC-family emulator image.",
    status: "downloading",
    archiveSizeBytes: 1048576000,
    checksum: "def456"
  },
  {
    relativePath: "system-image/HarmonyOS-6.0.1/wearable",
    displayName: "System-image-wearable",
    apiVersion: 21,
    deviceType: "wearable",
    version: "6.0.0.112",
    platformVersion: "6.0.1",
    guestVersion: "HarmonyOS 6.0.1",
    releaseType: "Release",
    description: "Wearable preview image.",
    status: "available",
    archiveSizeBytes: 268435456,
    checksum: "ghi789"
  }
];

const jobs: EmulatorDownloadJobSummary[] = [
  {
    jobId: "job-1",
    imageRelativePath: "system-image/HarmonyOS-6.0.1/pc_all",
    stage: "download",
    status: "running",
    progress: 48,
    increment: 4,
    network: 12.6,
    unit: "MB",
    reset: false
  }
];

const devices: EmulatorDeviceSummary[] = [
  {
    name: "Mate_X5_Emulator",
    instancePath: "/Users/demo/Library/OpenHarmony/deployed/Mate_X5_Emulator",
    deviceType: "foldable",
    model: "Mate X5",
    apiVersion: 21,
    showVersion: "HarmonyOS 6.0.1(21)",
    storageSizeBytes: 2147483648,
    snapshotBase64: null
  },
  {
    name: "MateBook_Hybrid_Emulator",
    instancePath: "/Users/demo/Library/OpenHarmony/deployed/MateBook_Hybrid_Emulator",
    deviceType: "2in1",
    model: "MateBook Hybrid",
    apiVersion: 21,
    showVersion: "HarmonyOS 6.0.1(21)",
    storageSizeBytes: 3221225472,
    snapshotBase64: null
  }
];

const createOptions: EmulatorCreateDeviceOptionsResult = {
  imageRelativePath: "system-image/HarmonyOS-6.0.1/phone_all",
  productPresets: [
    {
      name: "P40",
      deviceType: "Phone",
      screenWidth: "1080",
      screenHeight: "2340",
      screenDiagonal: "6.1",
      screenDensity: "480",
      defaultCpuCores: 4,
      defaultMemoryRamMb: 4096,
      defaultDataDiskMb: 6144
    },
    {
      name: "Mate X5",
      deviceType: "Foldable",
      screenWidth: "2224",
      screenHeight: "2496",
      screenDiagonal: "7.85",
      screenDensity: "426",
      defaultCpuCores: 4,
      defaultMemoryRamMb: 8192,
      defaultDataDiskMb: 6144
    }
  ]
};

const mockClient = {
  invoke: async (action: InvokeAction) => {
    switch (action) {
      case "host.getCapabilities":
        return { capabilities };
      case "emulator.getEnvironment":
        return environment;
      case "emulator.listImages":
        return { images };
      case "emulator.listDevices":
        return { devices };
      case "emulator.listDownloadJobs":
        return { jobs };
      case "emulator.getCreateDeviceOptions":
        return createOptions;
      case "emulator.downloadImage":
        return { jobId: "job-1" };
      case "emulator.createDevice":
        return { device: devices[0] };
      case "emulator.startDevice":
      case "emulator.stopDevice":
      case "emulator.deleteDevice":
        return { name: devices[0].name };
      case "hdc.listTargets":
        return { targets: ["127.0.0.1:8710"] };
      default:
        return {};
    }
  },
  onMessage: () => () => {}
} as unknown as HarmonyWebSocketClient;

const meta: Meta<typeof EmulatorsPanel> = {
  title: "Features/EmulatorsPanel",
  component: EmulatorsPanel,
  parameters: {
    layout: "fullscreen"
  }
};

export default meta;

type Story = StoryObj<typeof EmulatorsPanel>;

export const Loaded: Story = {
  args: {
    client: mockClient,
    connectionState: "open",
    hdcAvailable: true,
    currentHdcDevices: ["127.0.0.1:8710"],
    selectHdcDevice: () => {}
  }
};
