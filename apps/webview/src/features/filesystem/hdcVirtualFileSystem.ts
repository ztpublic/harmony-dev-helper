import type { HarmonyWebSocketClient } from "@harmony/webview-bridge";
import type { VfsEntry, VirtualFileSystem } from "./types";

interface CreateHdcVirtualFileSystemArgs {
  client: HarmonyWebSocketClient;
  connectKey: string;
  includeHidden?: boolean;
}

const FILE_TRANSFER_TIMEOUT_MS = 300_000;

function normalizeDevicePath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === "/") {
    return "/";
  }

  return trimmed.endsWith("/") ? trimmed.slice(0, -1) : trimmed;
}

function normalizeLocalPath(path: string): string {
  return path.trim();
}

export function createHdcVirtualFileSystem({
  client,
  connectKey,
  includeHidden = true
}: CreateHdcVirtualFileSystemArgs): VirtualFileSystem {
  return {
    async listDirectory(path: string): Promise<readonly VfsEntry[]> {
      const normalizedPath = normalizeDevicePath(path);
      const result = await client.invoke("hdc.fs.list", {
        connectKey,
        path: normalizedPath,
        includeHidden
      });

      return result.entries.map((entry) => ({
        path: entry.path,
        name: entry.name,
        kind: entry.kind
      }));
    },
    async uploadFile(localPath: string, remoteDirectory: string): Promise<{ remotePath: string }> {
      const normalizedLocalPath = normalizeLocalPath(localPath);
      const normalizedRemoteDirectory = normalizeDevicePath(remoteDirectory);
      const result = await client.invoke(
        "hdc.fs.upload",
        {
          connectKey,
          localPath: normalizedLocalPath,
          remoteDirectory: normalizedRemoteDirectory
        },
        {
          timeoutMs: FILE_TRANSFER_TIMEOUT_MS
        }
      );

      return {
        remotePath: result.remotePath
      };
    },
    async downloadFile(remotePath: string, localDirectory: string): Promise<{ localPath: string }> {
      const normalizedRemotePath = normalizeDevicePath(remotePath);
      const normalizedLocalDirectory = normalizeLocalPath(localDirectory);
      const result = await client.invoke(
        "hdc.fs.download",
        {
          connectKey,
          remotePath: normalizedRemotePath,
          localDirectory: normalizedLocalDirectory
        },
        {
          timeoutMs: FILE_TRANSFER_TIMEOUT_MS
        }
      );

      return {
        localPath: result.localPath
      };
    },
    async deletePath(path: string): Promise<{ deletedPath: string }> {
      const normalizedPath = normalizeDevicePath(path);
      const result = await client.invoke(
        "hdc.fs.delete",
        {
          connectKey,
          path: normalizedPath
        },
        {
          timeoutMs: FILE_TRANSFER_TIMEOUT_MS
        }
      );

      return {
        deletedPath: result.deletedPath
      };
    }
  };
}
