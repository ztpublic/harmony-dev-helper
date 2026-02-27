import type { Meta, StoryObj } from "@storybook/react";
import { DeviceSelector } from "./DeviceSelector";

const meta: Meta<typeof DeviceSelector> = {
  title: "HDC/DeviceSelector",
  component: DeviceSelector,
  args: {
    host: "tauri",
    connectionState: "open",
    status: "ready",
    isSupported: true,
    devices: ["FMR0223C13000649", "127.0.0.1:5555"],
    selectedDevice: "FMR0223C13000649",
    isRefreshing: false,
    onRefresh: () => undefined,
    onSelectDevice: () => undefined
  }
};

export default meta;

type Story = StoryObj<typeof DeviceSelector>;

export const Ready: Story = {};

export const UnsupportedHost: Story = {
  args: {
    host: "vscode",
    isSupported: false,
    devices: [],
    selectedDevice: null,
    status: "unsupported"
  }
};

export const Loading: Story = {
  args: {
    devices: [],
    selectedDevice: null,
    status: "loading"
  }
};

export const Error: Story = {
  args: {
    devices: [],
    selectedDevice: null,
    status: "error",
    errorMessage: "[HDC_ERROR] failed to connect to hdc server"
  }
};
