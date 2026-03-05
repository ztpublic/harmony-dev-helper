import type { Meta, StoryObj } from "@storybook/react";
import { useMemo, useState } from "react";
import { FileSystem } from "./FileSystem";
import type { VfsEntry, VirtualFileSystem } from "./types";

type MockFileTree = Record<string, readonly VfsEntry[]>;

type MockVfsOptions = {
  tree: MockFileTree;
  failPaths?: readonly string[];
  minDelayMs?: number;
  maxDelayMs?: number;
};

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    window.setTimeout(resolve, ms);
  });
}

function randomDelay(minDelayMs: number, maxDelayMs: number): number {
  if (maxDelayMs <= minDelayMs) {
    return minDelayMs;
  }

  return Math.floor(Math.random() * (maxDelayMs - minDelayMs + 1)) + minDelayMs;
}

function createMockVirtualFileSystem({
  tree,
  failPaths = [],
  minDelayMs = 150,
  maxDelayMs = 300
}: MockVfsOptions): VirtualFileSystem {
  const failPathSet = new Set(failPaths);

  return {
    async listDirectory(path: string): Promise<readonly VfsEntry[]> {
      await delay(randomDelay(minDelayMs, maxDelayMs));

      if (failPathSet.has(path)) {
        throw new Error(`Mock error: failed to load ${path}`);
      }

      const entries = tree[path];
      if (!entries) {
        throw new Error(`Directory not found: ${path}`);
      }

      return entries.map((entry) => ({ ...entry }));
    },
    async uploadFile(localPath: string, remoteDirectory: string): Promise<{ remotePath: string }> {
      return {
        remotePath: `${remoteDirectory.replace(/\/$/, "")}/${localPath.split(/[\\/]/).pop() ?? "file"}`
      };
    },
    async downloadFile(remotePath: string, localDirectory: string): Promise<{ localPath: string }> {
      const fileName = remotePath.split("/").filter(Boolean).pop() ?? "downloaded.file";
      return {
        localPath: `${localDirectory.replace(/[\\/]+$/, "")}/${fileName}`
      };
    },
    async deletePath(path: string): Promise<{ deletedPath: string }> {
      return {
        deletedPath: path
      };
    }
  };
}

function createLargeDirectoryTree(fileCount: number): MockFileTree {
  const rootEntries: VfsEntry[] = [
    { path: "/bulk", name: "bulk", kind: "directory" },
    { path: "/snapshots", name: "snapshots", kind: "directory" }
  ];

  const bulkEntries: VfsEntry[] = [];
  for (let index = 1; index <= fileCount; index += 1) {
    bulkEntries.push({
      path: `/bulk/log-${String(index).padStart(4, "0")}.txt`,
      name: `log-${String(index).padStart(4, "0")}.txt`,
      kind: "file",
      sizeBytes: 8_192 + index
    });
  }

  return {
    "/": rootEntries,
    "/bulk": bulkEntries,
    "/snapshots": [
      { path: "/snapshots/2026-03-01", name: "2026-03-01", kind: "directory" },
      { path: "/snapshots/2026-03-02", name: "2026-03-02", kind: "directory" }
    ],
    "/snapshots/2026-03-01": [
      { path: "/snapshots/2026-03-01/report.json", name: "report.json", kind: "file" }
    ],
    "/snapshots/2026-03-02": [
      { path: "/snapshots/2026-03-02/report.json", name: "report.json", kind: "file" }
    ]
  };
}

const HARMONY_TREE: MockFileTree = {
  "/": [
    { path: "/data", name: "data", kind: "directory" },
    { path: "/system", name: "system", kind: "directory" },
    { path: "/storage", name: "storage", kind: "directory" },
    { path: "/init.cfg", name: "init.cfg", kind: "file", sizeBytes: 2912 }
  ],
  "/data": [
    { path: "/data/log", name: "log", kind: "directory" },
    { path: "/data/app", name: "app", kind: "directory" },
    { path: "/data/service", name: "service", kind: "directory" }
  ],
  "/data/log": [
    { path: "/data/log/hilog", name: "hilog", kind: "directory" },
    { path: "/data/log/ability", name: "ability", kind: "directory" }
  ],
  "/data/log/hilog": [
    {
      path: "/data/log/hilog/hilog.2026-03-05.txt",
      name: "hilog.2026-03-05.txt",
      kind: "file",
      sizeBytes: 1024 * 128
    },
    {
      path: "/data/log/hilog/hilog.2026-03-04.txt",
      name: "hilog.2026-03-04.txt",
      kind: "file",
      sizeBytes: 1024 * 96
    }
  ],
  "/data/log/ability": [
    {
      path: "/data/log/ability/device_runtime.log",
      name: "device_runtime.log",
      kind: "file",
      sizeBytes: 1024 * 48
    }
  ],
  "/data/app": [
    { path: "/data/app/el2", name: "el2", kind: "directory" }
  ],
  "/data/app/el2": [
    {
      path: "/data/app/el2/100/base/com.example.harmony",
      name: "com.example.harmony",
      kind: "directory"
    }
  ],
  "/data/app/el2/100/base/com.example.harmony": [
    {
      path: "/data/app/el2/100/base/com.example.harmony/files",
      name: "files",
      kind: "directory"
    },
    {
      path: "/data/app/el2/100/base/com.example.harmony/cache",
      name: "cache",
      kind: "directory"
    }
  ],
  "/data/app/el2/100/base/com.example.harmony/files": [
    {
      path: "/data/app/el2/100/base/com.example.harmony/files/session.db",
      name: "session.db",
      kind: "file",
      sizeBytes: 1024 * 24
    }
  ],
  "/data/app/el2/100/base/com.example.harmony/cache": [],
  "/data/service": [
    {
      path: "/data/service/el1/public",
      name: "el1",
      kind: "directory"
    }
  ],
  "/data/service/el1/public": [
    {
      path: "/data/service/el1/public/databases",
      name: "databases",
      kind: "directory"
    }
  ],
  "/data/service/el1/public/databases": [
    {
      path: "/data/service/el1/public/databases/system.db",
      name: "system.db",
      kind: "file",
      sizeBytes: 1024 * 45
    }
  ],
  "/system": [
    { path: "/system/bin", name: "bin", kind: "directory" },
    { path: "/system/lib64", name: "lib64", kind: "directory" }
  ],
  "/system/bin": [
    { path: "/system/bin/hdcd", name: "hdcd", kind: "file", sizeBytes: 1024 * 512 },
    { path: "/system/bin/hilogd", name: "hilogd", kind: "file", sizeBytes: 1024 * 384 }
  ],
  "/system/lib64": [
    {
      path: "/system/lib64/libhilog.z.so",
      name: "libhilog.z.so",
      kind: "file",
      sizeBytes: 1024 * 256
    }
  ],
  "/storage": [
    { path: "/storage/media", name: "media", kind: "directory" }
  ],
  "/storage/media": [
    {
      path: "/storage/media/DCIM",
      name: "DCIM",
      kind: "directory"
    }
  ],
  "/storage/media/DCIM": [
    {
      path: "/storage/media/DCIM/camera_0001.jpg",
      name: "camera_0001.jpg",
      kind: "file",
      sizeBytes: 1024 * 2048
    }
  ]
};

interface StoryHarnessProps {
  vfs: VirtualFileSystem;
  rootPath?: string;
  height?: number;
}

function StoryHarness({ vfs, rootPath = "/", height = 360 }: StoryHarnessProps) {
  const [selectedPath, setSelectedPath] = useState<string>("none");
  const [openedPath, setOpenedPath] = useState<string>("none");

  return (
    <div className="file-system-story">
      <FileSystem
        vfs={vfs}
        rootPath={rootPath}
        height={height}
        onSelectionChange={(entry) => {
          setSelectedPath(entry?.path ?? "none");
        }}
        onOpenFile={(entry) => {
          setOpenedPath(entry.path);
        }}
      />

      <section className="panel file-system-story-events" aria-live="polite">
        <p className="kicker">Story Events</p>
        <div className="file-system-story-event-row">
          <span className="file-system-story-event-label">Selected</span>
          <code className="file-system-story-event-value">{selectedPath}</code>
        </div>
        <div className="file-system-story-event-row">
          <span className="file-system-story-event-label">Opened</span>
          <code className="file-system-story-event-value">{openedPath}</code>
        </div>
      </section>
    </div>
  );
}

const meta: Meta<typeof FileSystem> = {
  title: "Files/FileSystem",
  component: FileSystem,
  parameters: {
    layout: "padded"
  },
  argTypes: {
    vfs: {
      control: false
    },
    onSelectionChange: {
      table: {
        disable: true
      }
    },
    onOpenFile: {
      table: {
        disable: true
      }
    }
  }
};

export default meta;

type Story = StoryObj<typeof FileSystem>;

function DefaultStory(args: { rootPath?: string; height?: number }) {
  const vfs = useMemo(
    () =>
      createMockVirtualFileSystem({
        tree: HARMONY_TREE,
        minDelayMs: 150,
        maxDelayMs: 300
      }),
    []
  );

  return <StoryHarness vfs={vfs} rootPath={args.rootPath} height={args.height} />;
}

function ErrorStateStory(args: { rootPath?: string; height?: number }) {
  const vfs = useMemo(
    () =>
      createMockVirtualFileSystem({
        tree: HARMONY_TREE,
        failPaths: ["/data/log"],
        minDelayMs: 160,
        maxDelayMs: 280
      }),
    []
  );

  return <StoryHarness vfs={vfs} rootPath={args.rootPath} height={args.height} />;
}

function LargeDirectoryStory(args: { rootPath?: string; height?: number }) {
  const vfs = useMemo(
    () =>
      createMockVirtualFileSystem({
        tree: createLargeDirectoryTree(1200),
        minDelayMs: 170,
        maxDelayMs: 260
      }),
    []
  );

  return <StoryHarness vfs={vfs} rootPath={args.rootPath} height={args.height} />;
}

export const Default: Story = {
  args: {
    rootPath: "/",
    height: 360
  },
  render: (args) => <DefaultStory rootPath={args.rootPath} height={args.height} />
};

export const ErrorState: Story = {
  args: {
    rootPath: "/",
    height: 360
  },
  render: (args) => <ErrorStateStory rootPath={args.rootPath} height={args.height} />
};

export const LargeDirectory: Story = {
  args: {
    rootPath: "/",
    height: 420
  },
  render: (args) => <LargeDirectoryStory rootPath={args.rootPath} height={args.height} />
};
