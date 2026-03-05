import type { HarmonyWebSocketClient } from "@harmony/webview-bridge";
import type { VfsEntry, VirtualFileSystem } from "./types";

interface CreateHdcVirtualFileSystemArgs {
  client: HarmonyWebSocketClient;
  connectKey: string;
  includeHidden?: boolean;
}

function normalizeDirectoryPath(path: string): string {
  const trimmed = path.trim();
  if (!trimmed || trimmed === "/") {
    return "/";
  }

  return trimmed.endsWith("/") ? trimmed.slice(0, -1) : trimmed;
}

export function createHdcVirtualFileSystem({
  client,
  connectKey,
  includeHidden = true
}: CreateHdcVirtualFileSystemArgs): VirtualFileSystem {
  return {
    async listDirectory(path: string): Promise<readonly VfsEntry[]> {
      const normalizedPath = normalizeDirectoryPath(path);
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
    }
  };
}
