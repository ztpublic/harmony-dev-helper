import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Tree,
  type NodeApi,
  type NodeRendererProps,
  type RowRendererProps,
  type TreeApi
} from "react-arborist";
import defaultFileIcon from "./icons/default-file.svg";
import defaultFolderIcon from "./icons/default-folder.svg";
import defaultFolderOpenedIcon from "./icons/default-folder-opened.svg";
import type { FileSystemProps, VfsEntry } from "./types";

type DirectoryLoadState = "unloaded" | "loading" | "loaded" | "error";

type FileSystemNodeBase = VfsEntry & {
  id: string;
};

type FileSystemDirectoryNode = FileSystemNodeBase & {
  kind: "directory";
  children: FileSystemNode[];
  loadState: DirectoryLoadState;
  errorMessage?: string;
};

type FileSystemFileNode = FileSystemNodeBase & {
  kind: "file";
};

type FileSystemNode = FileSystemDirectoryNode | FileSystemFileNode;

type LoadDirectoryOptions = {
  force?: boolean;
  version?: number;
};

type FileSystemContextMenuState = {
  x: number;
  y: number;
  entry: VfsEntry;
};

const ROOT_PATH_DEFAULT = "/";
const ROOT_REQUEST_KEY = "__root__";
const TREE_HEIGHT_DEFAULT = 360;
const TREE_ROW_HEIGHT = 26;
const TREE_INDENT = 16;
const RECENT_EXPANDED_DIRECTORY_PATHS_LIMIT = 10;
const PATH_NAVIGATION_POLL_INTERVAL_MS = 40;
const PATH_NAVIGATION_TIMEOUT_MS = 8000;

const NAME_COLLATOR = new Intl.Collator(undefined, {
  sensitivity: "base",
  numeric: true
});

function toErrorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function fallbackCopyText(text: string): boolean {
  if (typeof document === "undefined" || !document.body) {
    return false;
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.left = "-9999px";
  textarea.style.top = "0";
  textarea.style.opacity = "0";
  textarea.style.pointerEvents = "none";

  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();

  let copied = false;
  try {
    copied = document.execCommand("copy");
  } catch {
    copied = false;
  }

  document.body.removeChild(textarea);
  return copied;
}

async function copyTextToClipboard(text: string): Promise<boolean> {
  if (!text) {
    return false;
  }

  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // fall through to legacy fallback
    }
  }

  return fallbackCopyText(text);
}

function toPublicEntry(node: FileSystemNode): VfsEntry {
  const { path, name, kind, sizeBytes, modifiedAtMs } = node;
  return {
    path,
    name,
    kind,
    ...(sizeBytes !== undefined ? { sizeBytes } : {}),
    ...(modifiedAtMs !== undefined ? { modifiedAtMs } : {})
  };
}

function compareEntries(a: VfsEntry, b: VfsEntry): number {
  if (a.kind !== b.kind) {
    return a.kind === "directory" ? -1 : 1;
  }

  const nameCompare = NAME_COLLATOR.compare(a.name, b.name);
  if (nameCompare !== 0) {
    return nameCompare;
  }

  return a.path.localeCompare(b.path);
}

function createFileSystemNode(entry: VfsEntry): FileSystemNode {
  if (entry.kind === "directory") {
    return {
      id: entry.path,
      path: entry.path,
      name: entry.name,
      kind: "directory",
      ...(entry.sizeBytes !== undefined ? { sizeBytes: entry.sizeBytes } : {}),
      ...(entry.modifiedAtMs !== undefined ? { modifiedAtMs: entry.modifiedAtMs } : {}),
      children: [],
      loadState: "unloaded"
    };
  }

  return {
    id: entry.path,
    path: entry.path,
    name: entry.name,
    kind: "file",
    ...(entry.sizeBytes !== undefined ? { sizeBytes: entry.sizeBytes } : {}),
    ...(entry.modifiedAtMs !== undefined ? { modifiedAtMs: entry.modifiedAtMs } : {})
  };
}

function toTreeNodes(entries: readonly VfsEntry[]): FileSystemNode[] {
  return [...entries].sort(compareEntries).map(createFileSystemNode);
}

function findNodeByPath(nodes: readonly FileSystemNode[], path: string): FileSystemNode | null {
  for (const node of nodes) {
    if (node.path === path) {
      return node;
    }

    if (node.kind === "directory" && node.children.length > 0) {
      const nested = findNodeByPath(node.children, path);
      if (nested) {
        return nested;
      }
    }
  }

  return null;
}

function updateDirectoryNode(
  nodes: readonly FileSystemNode[],
  path: string,
  update: (node: FileSystemDirectoryNode) => FileSystemDirectoryNode
): FileSystemNode[] {
  let changed = false;

  const nextNodes = nodes.map((node) => {
    if (node.path === path && node.kind === "directory") {
      changed = true;
      return update(node);
    }

    if (node.kind === "directory" && node.children.length > 0) {
      const nextChildren = updateDirectoryNode(node.children, path, update);
      if (nextChildren !== node.children) {
        changed = true;
        return {
          ...node,
          children: nextChildren
        };
      }
    }

    return node;
  });

  return changed ? nextNodes : (nodes as FileSystemNode[]);
}

function normalizeTreeHeight(height: number | undefined): number {
  if (!Number.isFinite(height)) {
    return TREE_HEIGHT_DEFAULT;
  }

  return Math.max(180, Math.floor(height as number));
}

function normalizeAbsolutePath(path: string): string | null {
  const trimmed = path.trim();
  if (!trimmed || !trimmed.startsWith("/")) {
    return null;
  }

  const segments = trimmed.split("/").filter((segment) => segment.length > 0);
  if (segments.length === 0) {
    return "/";
  }

  return `/${segments.join("/")}`;
}

function isWithinRootPath(path: string, rootPath: string): boolean {
  if (rootPath === "/") {
    return true;
  }

  return path === rootPath || path.startsWith(`${rootPath}/`);
}

function getPathSegmentsRelativeToRoot(path: string, rootPath: string): string[] {
  if (path === rootPath) {
    return [];
  }

  const offset = rootPath === "/" ? 1 : rootPath.length + 1;
  const relativePath = path.slice(offset);
  return relativePath.split("/").filter((segment) => segment.length > 0);
}

function joinPath(parent: string, child: string): string {
  if (parent === "/") {
    return `/${child}`;
  }

  return `${parent}/${child}`;
}

function parentDirectoryPath(path: string): string {
  const normalized = normalizeAbsolutePath(path);
  if (!normalized || normalized === "/") {
    return "/";
  }

  const segments = normalized.split("/").filter((segment) => segment.length > 0);
  if (segments.length <= 1) {
    return "/";
  }

  return `/${segments.slice(0, -1).join("/")}`;
}

function normalizeRecentExpandedDirectoryPaths(
  paths: readonly string[],
  rootPath: string
): readonly string[] {
  const normalizedPaths: string[] = [];
  const seen = new Set<string>();

  for (const rawPath of paths) {
    const normalizedPath = normalizeAbsolutePath(rawPath);
    if (!normalizedPath || !isWithinRootPath(normalizedPath, rootPath) || seen.has(normalizedPath)) {
      continue;
    }

    seen.add(normalizedPath);
    normalizedPaths.push(normalizedPath);

    if (normalizedPaths.length >= RECENT_EXPANDED_DIRECTORY_PATHS_LIMIT) {
      break;
    }
  }

  return normalizedPaths;
}

function addRecentExpandedDirectoryPath(
  paths: readonly string[],
  path: string,
  limit = RECENT_EXPANDED_DIRECTORY_PATHS_LIMIT
): readonly string[] {
  const withoutDuplicate = paths.filter((existingPath) => existingPath !== path);
  return [path, ...withoutDuplicate].slice(0, Math.max(1, limit));
}

export function FileSystem({
  vfs,
  rootPath = ROOT_PATH_DEFAULT,
  height,
  uploadEnabled = false,
  downloadEnabled = false,
  pickUploadFiles,
  pickDownloadDirectory,
  recentExpandedDirectoryPaths,
  onRecentExpandedDirectoryPathsChange,
  onSelectionChange,
  onOpenFile
}: FileSystemProps) {
  const [treeData, setTreeData] = useState<FileSystemNode[]>([]);
  const [rootLoadState, setRootLoadState] = useState<DirectoryLoadState>("unloaded");
  const [rootErrorMessage, setRootErrorMessage] = useState<string>();
  const normalizedRootPath = normalizeAbsolutePath(rootPath) ?? ROOT_PATH_DEFAULT;
  const [internalRecentExpandedDirectoryPaths, setInternalRecentExpandedDirectoryPaths] = useState<
    readonly string[]
  >([]);
  const [isRecentDirectoryDropdownOpen, setIsRecentDirectoryDropdownOpen] = useState(false);
  const [pathInputValue, setPathInputValue] = useState(normalizedRootPath);
  const [pathNavigationError, setPathNavigationError] = useState<string>();
  const [isPathNavigationPending, setIsPathNavigationPending] = useState(false);
  const [isFileTransferPending, setIsFileTransferPending] = useState(false);
  const [contextMenu, setContextMenu] = useState<FileSystemContextMenuState | null>(null);

  const pathComboboxRef = useRef<HTMLDivElement | null>(null);
  const contextMenuRef = useRef<HTMLDivElement | null>(null);
  const recentExpandedDirectoryPathOptionsRef = useRef<readonly string[]>([]);
  const treeApiRef = useRef<TreeApi<FileSystemNode> | null>(null);
  const treeDataRef = useRef<readonly FileSystemNode[]>([]);
  const rootLoadStateRef = useRef<DirectoryLoadState>("unloaded");
  const rootErrorMessageRef = useRef<string | undefined>(undefined);
  const cacheVersionRef = useRef(0);
  const inFlightRef = useRef(new Set<string>());
  const onSelectionChangeRef = useRef(onSelectionChange);
  const onOpenFileRef = useRef(onOpenFile);

  const treeHeight = normalizeTreeHeight(height);
  const recentExpandedDirectoryPathOptions = useMemo(
    () =>
      normalizeRecentExpandedDirectoryPaths(
        recentExpandedDirectoryPaths ?? internalRecentExpandedDirectoryPaths,
        normalizedRootPath
      ),
    [recentExpandedDirectoryPaths, internalRecentExpandedDirectoryPaths, normalizedRootPath]
  );

  const updateRecentExpandedDirectoryPaths = useCallback(
    (nextPaths: readonly string[]) => {
      const normalizedPaths = normalizeRecentExpandedDirectoryPaths(nextPaths, normalizedRootPath);
      if (onRecentExpandedDirectoryPathsChange) {
        onRecentExpandedDirectoryPathsChange(normalizedPaths);
        return;
      }

      setInternalRecentExpandedDirectoryPaths(normalizedPaths);
    },
    [normalizedRootPath, onRecentExpandedDirectoryPathsChange]
  );

  useEffect(() => {
    recentExpandedDirectoryPathOptionsRef.current = recentExpandedDirectoryPathOptions;
  }, [recentExpandedDirectoryPathOptions]);

  const rememberExpandedDirectoryPath = useCallback(
    (directoryPath: string) => {
      const normalizedPath = normalizeAbsolutePath(directoryPath);
      if (!normalizedPath || !isWithinRootPath(normalizedPath, normalizedRootPath)) {
        return;
      }

      const nextPaths = addRecentExpandedDirectoryPath(
        recentExpandedDirectoryPathOptionsRef.current,
        normalizedPath
      );
      recentExpandedDirectoryPathOptionsRef.current = nextPaths;
      updateRecentExpandedDirectoryPaths(nextPaths);
    },
    [normalizedRootPath, updateRecentExpandedDirectoryPaths]
  );

  const closeRecentDirectoryDropdown = useCallback(() => {
    setIsRecentDirectoryDropdownOpen(false);
  }, []);

  useEffect(() => {
    treeDataRef.current = treeData;
  }, [treeData]);

  useEffect(() => {
    rootLoadStateRef.current = rootLoadState;
  }, [rootLoadState]);

  useEffect(() => {
    rootErrorMessageRef.current = rootErrorMessage;
  }, [rootErrorMessage]);

  useEffect(() => {
    setPathInputValue(normalizedRootPath);
    setPathNavigationError(undefined);
    closeRecentDirectoryDropdown();
  }, [closeRecentDirectoryDropdown, normalizedRootPath]);

  useEffect(() => {
    if (!onRecentExpandedDirectoryPathsChange && recentExpandedDirectoryPaths === undefined) {
      setInternalRecentExpandedDirectoryPaths((current) =>
        normalizeRecentExpandedDirectoryPaths(current, normalizedRootPath)
      );
    }
  }, [normalizedRootPath, onRecentExpandedDirectoryPathsChange, recentExpandedDirectoryPaths]);

  useEffect(() => {
    onSelectionChangeRef.current = onSelectionChange;
  }, [onSelectionChange]);

  useEffect(() => {
    onOpenFileRef.current = onOpenFile;
  }, [onOpenFile]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const menuElement = contextMenuRef.current;
    if (!menuElement) {
      return;
    }

    const bounds = menuElement.getBoundingClientRect();
    const viewportWidth = window.innerWidth;
    const viewportHeight = window.innerHeight;
    const margin = 8;
    let nextX = contextMenu.x;
    let nextY = contextMenu.y;

    if (nextX + bounds.width + margin > viewportWidth) {
      nextX = Math.max(margin, viewportWidth - bounds.width - margin);
    }

    if (nextY + bounds.height + margin > viewportHeight) {
      nextY = Math.max(margin, viewportHeight - bounds.height - margin);
    }

    if (nextX === contextMenu.x && nextY === contextMenu.y) {
      return;
    }

    setContextMenu((current) => {
      if (!current) {
        return current;
      }

      if (current.x === nextX && current.y === nextY) {
        return current;
      }

      return {
        ...current,
        x: nextX,
        y: nextY
      };
    });
  }, [contextMenu]);

  useEffect(() => {
    if (!contextMenu) {
      return;
    }

    const closeContextMenu = () => {
      setContextMenu(null);
    };

    const onMouseDown = (event: MouseEvent) => {
      if (event.button === 2) {
        return;
      }

      const target = event.target as Node | null;
      if (!target || contextMenuRef.current?.contains(target)) {
        return;
      }

      closeContextMenu();
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeContextMenu();
      }
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", closeContextMenu);

    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", closeContextMenu);
    };
  }, [contextMenu]);

  useEffect(() => {
    if (!isRecentDirectoryDropdownOpen) {
      return;
    }

    const onMouseDown = (event: MouseEvent) => {
      const target = event.target as Node | null;
      if (!target || pathComboboxRef.current?.contains(target)) {
        return;
      }

      closeRecentDirectoryDropdown();
    };

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeRecentDirectoryDropdown();
      }
    };

    document.addEventListener("mousedown", onMouseDown);
    document.addEventListener("keydown", onKeyDown);
    window.addEventListener("resize", closeRecentDirectoryDropdown);

    return () => {
      document.removeEventListener("mousedown", onMouseDown);
      document.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("resize", closeRecentDirectoryDropdown);
    };
  }, [closeRecentDirectoryDropdown, isRecentDirectoryDropdownOpen]);

  const loadDirectory = useCallback(
    async (directoryPath: string, options: LoadDirectoryOptions = {}) => {
      const version = options.version ?? cacheVersionRef.current;
      const target = findNodeByPath(treeDataRef.current, directoryPath);

      if (!target || target.kind !== "directory") {
        return;
      }

      if (!options.force && (target.loadState === "loaded" || target.loadState === "loading")) {
        return;
      }

      const requestKey = `${version}:${directoryPath}`;
      if (inFlightRef.current.has(requestKey)) {
        return;
      }

      inFlightRef.current.add(requestKey);

      setTreeData((current) =>
        updateDirectoryNode(current, directoryPath, (node) => ({
          ...node,
          loadState: "loading",
          errorMessage: undefined
        }))
      );

      try {
        const entries = await vfs.listDirectory(directoryPath);

        if (cacheVersionRef.current !== version) {
          return;
        }

        const children = toTreeNodes(entries);
        setTreeData((current) =>
          updateDirectoryNode(current, directoryPath, (node) => ({
            ...node,
            loadState: "loaded",
            errorMessage: undefined,
            children
          }))
        );
      } catch (error) {
        if (cacheVersionRef.current !== version) {
          return;
        }

        const errorMessage = toErrorMessage(error);
        setTreeData((current) =>
          updateDirectoryNode(current, directoryPath, (node) => ({
            ...node,
            loadState: "error",
            errorMessage
          }))
        );
      } finally {
        inFlightRef.current.delete(requestKey);
      }
    },
    [vfs]
  );

  const loadRootDirectory = useCallback(
    async (options: LoadDirectoryOptions = {}) => {
      const version = options.version ?? cacheVersionRef.current;

      if (!options.force && (rootLoadStateRef.current === "loaded" || rootLoadStateRef.current === "loading")) {
        return;
      }

      const requestKey = `${version}:${ROOT_REQUEST_KEY}`;
      if (inFlightRef.current.has(requestKey)) {
        return;
      }

      inFlightRef.current.add(requestKey);
      setRootLoadState("loading");
      setRootErrorMessage(undefined);

      try {
        const entries = await vfs.listDirectory(normalizedRootPath);

        if (cacheVersionRef.current !== version) {
          return;
        }

        setTreeData(toTreeNodes(entries));
        setRootLoadState("loaded");
      } catch (error) {
        if (cacheVersionRef.current !== version) {
          return;
        }

        setTreeData([]);
        setRootLoadState("error");
        setRootErrorMessage(toErrorMessage(error));
      } finally {
        inFlightRef.current.delete(requestKey);
      }
    },
    [normalizedRootPath, vfs]
  );

  const waitForRootLoaded = useCallback(async (version: number) => {
    const startTime = Date.now();

    while (cacheVersionRef.current === version) {
      const currentState = rootLoadStateRef.current;
      if (currentState === "loaded") {
        return;
      }

      if (currentState === "error") {
        throw new Error(rootErrorMessageRef.current ?? "Failed to load root directory.");
      }

      if (Date.now() - startTime >= PATH_NAVIGATION_TIMEOUT_MS) {
        throw new Error("Timed out while loading root directory.");
      }

      await new Promise<void>((resolve) => {
        setTimeout(resolve, PATH_NAVIGATION_POLL_INTERVAL_MS);
      });
    }

    throw new Error("Explorer state changed while navigating to path.");
  }, []);

  const waitForDirectoryLoaded = useCallback(async (directoryPath: string, version: number) => {
    const startTime = Date.now();

    while (cacheVersionRef.current === version) {
      const node = findNodeByPath(treeDataRef.current, directoryPath);
      if (!node || node.kind !== "directory") {
        throw new Error(`Path not found: ${directoryPath}`);
      }

      if (node.loadState === "loaded") {
        return;
      }

      if (node.loadState === "error") {
        throw new Error(node.errorMessage ?? `Failed to load ${directoryPath}`);
      }

      if (Date.now() - startTime >= PATH_NAVIGATION_TIMEOUT_MS) {
        throw new Error(`Timed out while loading ${directoryPath}.`);
      }

      await new Promise<void>((resolve) => {
        setTimeout(resolve, PATH_NAVIGATION_POLL_INTERVAL_MS);
      });
    }

    throw new Error("Explorer state changed while navigating to path.");
  }, []);

  const navigateToPath = useCallback(
    async (rawPath: string) => {
      const targetPath = normalizeAbsolutePath(rawPath);
      if (!targetPath) {
        setPathNavigationError("Enter an absolute path (for example, `/data`).");
        return;
      }

      if (!isWithinRootPath(targetPath, normalizedRootPath)) {
        setPathNavigationError(`Path must be inside ${normalizedRootPath}.`);
        return;
      }

      const version = cacheVersionRef.current;
      setPathNavigationError(undefined);
      setPathInputValue(targetPath);
      setIsPathNavigationPending(true);
      closeRecentDirectoryDropdown();
      setContextMenu(null);
      let lastExpandedDirectoryPath: string | null = null;

      try {
        if (rootLoadStateRef.current !== "loaded") {
          await loadRootDirectory({
            force: rootLoadStateRef.current === "error",
            version
          });
        }

        await waitForRootLoaded(version);

        const segments = getPathSegmentsRelativeToRoot(targetPath, normalizedRootPath);
        let parentPath = normalizedRootPath;

        for (let index = 0; index < segments.length; index += 1) {
          const segment = segments[index];
          if (!segment) {
            continue;
          }

          const currentPath = joinPath(parentPath, segment);
          const isLastSegment = index === segments.length - 1;
          const currentNode = findNodeByPath(treeDataRef.current, currentPath);

          if (!currentNode) {
            throw new Error(`Path not found: ${currentPath}`);
          }

          if (!isLastSegment) {
            if (currentNode.kind !== "directory") {
              throw new Error(`Cannot expand through file: ${currentPath}`);
            }

            const treeNode = treeApiRef.current?.get(currentPath);
            if (treeNode && !treeNode.isOpen) {
              treeNode.open();
            }

            if (currentNode.loadState !== "loaded") {
              await loadDirectory(currentPath, {
                force: currentNode.loadState === "error",
                version
              });
            }

            await waitForDirectoryLoaded(currentPath, version);
            lastExpandedDirectoryPath = currentPath;
          } else if (currentNode.kind === "directory") {
            const treeNode = treeApiRef.current?.get(currentPath);
            if (treeNode && !treeNode.isOpen) {
              treeNode.open();
            }

            if (currentNode.loadState !== "loaded") {
              await loadDirectory(currentPath, {
                force: currentNode.loadState === "error",
                version
              });
            }

            await waitForDirectoryLoaded(currentPath, version);
            lastExpandedDirectoryPath = currentPath;
          }

          parentPath = currentPath;
        }

        if (targetPath === normalizedRootPath) {
          treeApiRef.current?.deselectAll();
          onSelectionChangeRef.current?.(null);
          return;
        }

        const targetNode = treeApiRef.current?.get(targetPath);
        if (!targetNode) {
          throw new Error(`Path not found: ${targetPath}`);
        }

        targetNode.select();
        targetNode.focus();
        void treeApiRef.current?.scrollTo(targetPath);

        if (lastExpandedDirectoryPath) {
          rememberExpandedDirectoryPath(lastExpandedDirectoryPath);
        }
      } catch (error) {
        setPathNavigationError(toErrorMessage(error));
      } finally {
        setIsPathNavigationPending(false);
      }
    },
    [
      loadDirectory,
      loadRootDirectory,
      normalizedRootPath,
      closeRecentDirectoryDropdown,
      rememberExpandedDirectoryPath,
      waitForDirectoryLoaded,
      waitForRootLoaded
    ]
  );

  const refresh = useCallback(() => {
    const nextVersion = cacheVersionRef.current + 1;
    cacheVersionRef.current = nextVersion;
    inFlightRef.current.clear();
    setTreeData([]);
    setRootLoadState("unloaded");
    setRootErrorMessage(undefined);
    setPathInputValue(normalizedRootPath);
    setPathNavigationError(undefined);
    setIsPathNavigationPending(false);
    setIsFileTransferPending(false);
    closeRecentDirectoryDropdown();
    setContextMenu(null);
    onSelectionChangeRef.current?.(null);
    void loadRootDirectory({
      force: true,
      version: nextVersion
    });
  }, [closeRecentDirectoryDropdown, loadRootDirectory, normalizedRootPath]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleToggle = useCallback(
    (id: string) => {
      const toggledNode = treeApiRef.current?.get(id);
      if (!toggledNode || toggledNode.data.kind !== "directory") {
        return;
      }

      if (!toggledNode.isOpen) {
        return;
      }

      const shouldLoad =
        toggledNode.data.loadState === "unloaded" || toggledNode.data.loadState === "error";
      const directoryPath = toggledNode.data.path;
      const forceReload = toggledNode.data.loadState === "error";

      if (!shouldLoad) {
        rememberExpandedDirectoryPath(directoryPath);
        return;
      }

      const version = cacheVersionRef.current;
      const loadExpandedDirectory = async () => {
        await loadDirectory(directoryPath, {
          force: forceReload,
          version
        });

        await waitForDirectoryLoaded(directoryPath, version);
        rememberExpandedDirectoryPath(directoryPath);
      };

      void loadExpandedDirectory().catch(() => {
        // Directory errors are surfaced in-row by existing load state handling.
      });
    },
    [loadDirectory, rememberExpandedDirectoryPath, waitForDirectoryLoaded]
  );

  const handleSelect = useCallback(
    (nodes: NodeApi<FileSystemNode>[]) => {
      const selectedNode = nodes[0];
      if (!selectedNode) {
        // Arborist can emit transient empty selections while toggling folders.
        // Ignore these so we don't clobber the path input with internal root state.
        return;
      }

      const selectedEntry = toPublicEntry(selectedNode.data);
      setPathInputValue(selectedEntry.path);
      setPathNavigationError(undefined);
      closeRecentDirectoryDropdown();
      setContextMenu(null);
      onSelectionChangeRef.current?.(selectedEntry);
    },
    [closeRecentDirectoryDropdown]
  );

  const handleContextMenuCopyPath = useCallback(async () => {
    if (!contextMenu) {
      return;
    }

    const copied = await copyTextToClipboard(contextMenu.entry.path);
    setContextMenu(null);

    if (!copied) {
      setPathNavigationError("Failed to copy path to clipboard.");
      return;
    }

    setPathNavigationError(undefined);
  }, [contextMenu]);

  const handleContextMenuUpload = useCallback(async () => {
    if (!contextMenu || !uploadEnabled || !pickUploadFiles) {
      return;
    }

    const targetDirectoryPath =
      contextMenu.entry.kind === "directory"
        ? contextMenu.entry.path
        : parentDirectoryPath(contextMenu.entry.path);
    setContextMenu(null);
    setPathNavigationError(undefined);
    setIsFileTransferPending(true);

    try {
      const localPaths = await pickUploadFiles(targetDirectoryPath);
      if (!localPaths || localPaths.length === 0) {
        return;
      }

      const version = cacheVersionRef.current;
      for (const localPath of localPaths) {
        await vfs.uploadFile(localPath, targetDirectoryPath);
      }

      await loadDirectory(targetDirectoryPath, {
        force: true,
        version
      });
      await waitForDirectoryLoaded(targetDirectoryPath, version);
      setPathNavigationError(undefined);
    } catch (error) {
      setPathNavigationError(toErrorMessage(error));
    } finally {
      setIsFileTransferPending(false);
    }
  }, [contextMenu, loadDirectory, pickUploadFiles, uploadEnabled, vfs, waitForDirectoryLoaded]);

  const handleContextMenuDownload = useCallback(async () => {
    if (!contextMenu || contextMenu.entry.kind !== "file" || !downloadEnabled || !pickDownloadDirectory) {
      return;
    }

    const sourceFilePath = contextMenu.entry.path;
    setContextMenu(null);
    setPathNavigationError(undefined);
    setIsFileTransferPending(true);

    try {
      const localDirectory = await pickDownloadDirectory(sourceFilePath);
      if (!localDirectory) {
        return;
      }

      await vfs.downloadFile(sourceFilePath, localDirectory);
      setPathNavigationError(undefined);
    } catch (error) {
      setPathNavigationError(toErrorMessage(error));
    } finally {
      setIsFileTransferPending(false);
    }
  }, [contextMenu, downloadEnabled, pickDownloadDirectory, vfs]);

  const renderRow = useCallback(
    ({ node, attrs, innerRef, children }: RowRendererProps<FileSystemNode>) => {
      const className = [
        attrs.className,
        "file-system-row",
        node.isSelected ? "file-system-row-selected" : "",
        node.isFocused ? "file-system-row-focused" : ""
      ]
        .filter(Boolean)
        .join(" ");

      return (
        <div
          {...attrs}
          ref={innerRef}
          className={className}
          onFocus={(event) => {
            event.stopPropagation();
          }}
          onClick={(event) => {
            event.stopPropagation();
            closeRecentDirectoryDropdown();
            setContextMenu(null);
            node.select();

            if (node.data.kind === "directory") {
              node.toggle();
            }
          }}
          onDoubleClick={(event) => {
            event.stopPropagation();

            if (node.data.kind === "file") {
              onOpenFileRef.current?.(toPublicEntry(node.data));
            }
          }}
          onContextMenu={(event) => {
            event.preventDefault();
            event.stopPropagation();
            if (isFileTransferPending) {
              return;
            }

            const entry = toPublicEntry(node.data);
            node.select();
            setPathInputValue(entry.path);
            setPathNavigationError(undefined);
            closeRecentDirectoryDropdown();
            onSelectionChangeRef.current?.(entry);
            setContextMenu({
              x: event.clientX,
              y: event.clientY,
              entry
            });
          }}
        >
          {children}
        </div>
      );
    },
    [closeRecentDirectoryDropdown, isFileTransferPending]
  );

  const renderNode = useCallback(
    ({ node, style }: NodeRendererProps<FileSystemNode>) => {
      const entry = node.data;
      const isDirectory = entry.kind === "directory";

      return (
        <div style={style} className="file-system-node" title={entry.path}>
          {isDirectory ? (
            <span className="file-system-expander" aria-hidden="true">
              <svg
                className={`file-system-expander-icon ${node.isOpen ? "file-system-expander-icon-open" : ""}`}
                viewBox="0 0 16 16"
                aria-hidden="true"
                focusable="false"
              >
                <path d="M6 4.5L10 8L6 11.5" />
              </svg>
            </span>
          ) : (
            <span className="file-system-expander-placeholder" aria-hidden="true" />
          )}

          <span className="file-system-entry-icon" aria-hidden="true">
            <img
              className="file-system-entry-icon-image"
              src={
                isDirectory ? (node.isOpen ? defaultFolderOpenedIcon : defaultFolderIcon) : defaultFileIcon
              }
              alt=""
              draggable={false}
            />
          </span>

          <span className="file-system-node-label">{entry.name}</span>

          {isDirectory && entry.loadState === "loading" ? (
            <span className="file-system-node-meta">Loading...</span>
          ) : null}

          {isDirectory && entry.loadState === "error" ? (
            <>
              <span className="file-system-node-error" title={entry.errorMessage ?? "Failed to load directory"}>
                Load failed
              </span>
              <button
                type="button"
                className="file-system-retry-button"
                onClick={(event) => {
                  event.stopPropagation();
                  void loadDirectory(entry.path, { force: true });
                }}
              >
                Retry
              </button>
            </>
          ) : null}
        </div>
      );
    },
    [loadDirectory]
  );

  const showUploadAction =
    Boolean(uploadEnabled) &&
    Boolean(pickUploadFiles) &&
    Boolean(contextMenu);
  const showDownloadAction =
    Boolean(downloadEnabled) &&
    Boolean(pickDownloadDirectory) &&
    contextMenu?.entry.kind === "file";

  return (
    <section className="panel file-system-panel" aria-label="File system">
      <div className="file-system-header">
        <div className="file-system-path-combobox" ref={pathComboboxRef}>
          <input
            type="text"
            className="file-system-path-input"
            aria-label="Selected file path"
            value={pathInputValue}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            placeholder="Paste a path and press Enter"
            disabled={isFileTransferPending}
            onChange={(event) => {
              setPathInputValue(event.target.value);
              setPathNavigationError(undefined);
              closeRecentDirectoryDropdown();
            }}
            onKeyDown={(event) => {
              if (event.key === "ArrowDown") {
                event.preventDefault();
                setIsRecentDirectoryDropdownOpen(true);
                return;
              }

              if (event.key !== "Enter") {
                return;
              }

              event.preventDefault();
              void navigateToPath(pathInputValue);
            }}
          />
          <button
            type="button"
            className="file-system-path-dropdown-trigger"
            aria-label="Show recent expanded folders"
            aria-haspopup="listbox"
            aria-expanded={isRecentDirectoryDropdownOpen}
            onClick={() => {
              setIsRecentDirectoryDropdownOpen((current) => !current);
            }}
            disabled={isPathNavigationPending || isFileTransferPending}
          >
            <svg
              className={`file-system-path-dropdown-caret ${
                isRecentDirectoryDropdownOpen ? "file-system-path-dropdown-caret-open" : ""
              }`}
              viewBox="0 0 16 16"
              aria-hidden="true"
              focusable="false"
            >
              <path d="M4.5 6.5L8 10L11.5 6.5" />
            </svg>
          </button>

          {isRecentDirectoryDropdownOpen ? (
            <div className="file-system-path-dropdown" role="listbox" aria-label="Recent expanded folders">
              {recentExpandedDirectoryPathOptions.length === 0 ? (
                <div className="file-system-path-dropdown-empty" aria-disabled="true">
                  No recent folders
                </div>
              ) : (
                recentExpandedDirectoryPathOptions.map((recentPath) => (
                  <button
                    key={recentPath}
                    type="button"
                    className={`file-system-path-dropdown-item ${
                      recentPath === pathInputValue ? "file-system-path-dropdown-item-active" : ""
                    }`}
                    role="option"
                    aria-selected={recentPath === pathInputValue}
                    onClick={() => {
                      setPathInputValue(recentPath);
                      setPathNavigationError(undefined);
                      closeRecentDirectoryDropdown();
                      void navigateToPath(recentPath);
                    }}
                  >
                    {recentPath}
                  </button>
                ))
              )}
            </div>
          ) : null}
        </div>

        <button
          type="button"
          className="file-system-refresh-button"
          onClick={refresh}
          disabled={rootLoadState === "loading" || isPathNavigationPending || isFileTransferPending}
        >
          {rootLoadState === "loading" ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      {pathNavigationError ? (
        <p className="panel-message panel-message-error">{pathNavigationError}</p>
      ) : null}

      {rootLoadState === "error" ? (
        <div className="file-system-root-error">
          <p className="panel-message panel-message-error">
            {rootErrorMessage ?? "Failed to load root directory."}
          </p>
          <button type="button" className="file-system-root-retry" onClick={refresh}>
            Retry
          </button>
        </div>
      ) : null}

      {rootLoadState === "loading" && treeData.length === 0 ? (
        <p className="panel-message">Loading directory...</p>
      ) : null}

      {rootLoadState === "loaded" && treeData.length === 0 ? (
        <p className="panel-message">Directory is empty.</p>
      ) : null}

      {rootLoadState !== "error" && treeData.length > 0 ? (
        <div className="file-system-tree-shell">
          <Tree<FileSystemNode>
            ref={treeApiRef}
            data={treeData}
            width="100%"
            height={treeHeight}
            rowHeight={TREE_ROW_HEIGHT}
            indent={TREE_INDENT}
            openByDefault={false}
            disableDrag
            disableDrop
            disableEdit
            disableMultiSelection
            className="file-system-tree"
            rowClassName="file-system-tree-row"
            renderRow={renderRow}
            onToggle={handleToggle}
            onSelect={handleSelect}
          >
            {renderNode}
          </Tree>
        </div>
      ) : null}

      {contextMenu ? (
        <div
          ref={contextMenuRef}
          className="file-system-context-menu"
          role="menu"
          style={{
            left: `${contextMenu.x}px`,
            top: `${contextMenu.y}px`
          }}
        >
          {showUploadAction ? (
            <button
              type="button"
              className="file-system-context-menu-item"
              role="menuitem"
              onClick={() => {
                void handleContextMenuUpload();
              }}
            >
              Upload
            </button>
          ) : null}

          {showDownloadAction ? (
            <button
              type="button"
              className="file-system-context-menu-item"
              role="menuitem"
              onClick={() => {
                void handleContextMenuDownload();
              }}
            >
              Download
            </button>
          ) : null}

          <button
            type="button"
            className="file-system-context-menu-item"
            role="menuitem"
            onClick={() => {
              void handleContextMenuCopyPath();
            }}
          >
            Copy Path
          </button>
        </div>
      ) : null}
    </section>
  );
}
