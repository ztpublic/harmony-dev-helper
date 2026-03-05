export type VfsEntryKind = "directory" | "file";

export interface VfsEntry {
  path: string;
  name: string;
  kind: VfsEntryKind;
  sizeBytes?: number;
  modifiedAtMs?: number;
}

export interface VirtualFileSystem {
  listDirectory(path: string): Promise<readonly VfsEntry[]>;
  uploadFile(localPath: string, remoteDirectory: string): Promise<{ remotePath: string }>;
  downloadFile(remotePath: string, localDirectory: string): Promise<{ localPath: string }>;
}

export interface FileSystemProps {
  vfs: VirtualFileSystem;
  rootPath?: string;
  height?: number;
  uploadEnabled?: boolean;
  downloadEnabled?: boolean;
  pickUploadFiles?: (targetDirectoryPath: string) => Promise<readonly string[] | null>;
  pickDownloadDirectory?: (sourceFilePath: string) => Promise<string | null>;
  recentExpandedDirectoryPaths?: readonly string[];
  onRecentExpandedDirectoryPathsChange?: (paths: readonly string[]) => void;
  onSelectionChange?: (entry: VfsEntry | null) => void;
  onOpenFile?: (entry: VfsEntry) => void;
}
