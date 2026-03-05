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
}

export interface FileSystemProps {
  vfs: VirtualFileSystem;
  rootPath?: string;
  height?: number;
  onSelectionChange?: (entry: VfsEntry | null) => void;
  onOpenFile?: (entry: VfsEntry) => void;
}
