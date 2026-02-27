import type { Meta, StoryObj } from "@storybook/react";
import { ConnectionStatus } from "./ConnectionStatus";

const meta: Meta<typeof ConnectionStatus> = {
  title: "Bridge/ConnectionStatus",
  component: ConnectionStatus,
  args: {
    host: "tauri",
    wsUrl: "ws://127.0.0.1:8787",
    state: "open",
    lastMessageType: "event"
  }
};

export default meta;

type Story = StoryObj<typeof ConnectionStatus>;

export const Connected: Story = {};

export const Connecting: Story = {
  args: {
    state: "connecting",
    lastMessageType: undefined
  }
};

export const Error: Story = {
  args: {
    state: "error",
    lastMessageType: "error"
  }
};
