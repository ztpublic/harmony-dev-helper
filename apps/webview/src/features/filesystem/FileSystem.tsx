import { useCallback, useEffect, useRef, useState } from "react";
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
  path: string;
};

const ROOT_PATH_DEFAULT = "/";
const ROOT_REQUEST_KEY = "__root__";
const TREE_HEIGHT_DEFAULT = 360;
const TREE_ROW_HEIGHT = 26;
const TREE_INDENT = 16;
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

export function FileSystem({
  vfs,
  rootPath = ROOT_PATH_DEFAULT,
  height,
  onSelectionChange,
  onOpenFile
}: FileSystemProps) {
  const [treeData, setTreeData] = useState<FileSystemNode[]>([]);
  const [rootLoadState, setRootLoadState] = useState<DirectoryLoadState>("unloaded");
  const [rootErrorMessage, setRootErrorMessage] = useState<string>();
  const normalizedRootPath = normalizeAbsolutePath(rootPath) ?? ROOT_PATH_DEFAULT;
  const [pathInputValue, setPathInputValue] = useState(normalizedRootPath);
  const [pathNavigationError, setPathNavigationError] = useState<string>();
  const [isPathNavigationPending, setIsPathNavigationPending] = useState(false);
  const [contextMenu, setContextMenu] = useState<FileSystemContextMenuState | null>(null);

  const contextMenuRef = useRef<HTMLDivElement | null>(null);
  const treeApiRef = useRef<TreeApi<FileSystemNode> | null>(null);
  const treeDataRef = useRef<readonly FileSystemNode[]>([]);
  const rootLoadStateRef = useRef<DirectoryLoadState>("unloaded");
  const rootErrorMessageRef = useRef<string | undefined>(undefined);
  const cacheVersionRef = useRef(0);
  const inFlightRef = useRef(new Set<string>());
  const onSelectionChangeRef = useRef(onSelectionChange);
  const onOpenFileRef = useRef(onOpenFile);

  const treeHeight = normalizeTreeHeight(height);

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
  }, [normalizedRootPath]);

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
      setContextMenu(null);

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
      } catch (error) {
        setPathNavigationError(toErrorMessage(error));
      } finally {
        setIsPathNavigationPending(false);
      }
    },
    [loadDirectory, loadRootDirectory, normalizedRootPath, waitForDirectoryLoaded, waitForRootLoaded]
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
    setContextMenu(null);
    onSelectionChangeRef.current?.(null);
    void loadRootDirectory({
      force: true,
      version: nextVersion
    });
  }, [loadRootDirectory, normalizedRootPath]);

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

      if (shouldLoad) {
        void loadDirectory(toggledNode.data.path, {
          force: toggledNode.data.loadState === "error"
        });
      }
    },
    [loadDirectory]
  );

  const handleSelect = useCallback(
    (nodes: NodeApi<FileSystemNode>[]) => {
      const firstSelected = nodes[0];
      const selectedEntry = firstSelected ? toPublicEntry(firstSelected.data) : null;
      setPathInputValue(selectedEntry?.path ?? normalizedRootPath);
      setPathNavigationError(undefined);
      setContextMenu(null);
      onSelectionChangeRef.current?.(selectedEntry);
    },
    [normalizedRootPath]
  );

  const handleContextMenuCopyPath = useCallback(async () => {
    if (!contextMenu) {
      return;
    }

    const copied = await copyTextToClipboard(contextMenu.path);
    setContextMenu(null);

    if (!copied) {
      setPathNavigationError("Failed to copy path to clipboard.");
      return;
    }

    setPathNavigationError(undefined);
  }, [contextMenu]);

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
            setContextMenu(null);
            node.select();

            if (node.data.kind === "directory" && event.detail === 1) {
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
            const entry = toPublicEntry(node.data);
            node.select();
            setPathInputValue(entry.path);
            setPathNavigationError(undefined);
            onSelectionChangeRef.current?.(entry);
            setContextMenu({
              x: event.clientX,
              y: event.clientY,
              path: entry.path
            });
          }}
        >
          {children}
        </div>
      );
    },
    []
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

  return (
    <section className="panel file-system-panel" aria-label="File system">
      <div className="file-system-header">
        <input
          type="text"
          className="file-system-path-input"
          aria-label="Selected file path"
          value={pathInputValue}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          placeholder="Paste a path and press Enter"
          onChange={(event) => {
            setPathInputValue(event.target.value);
            setPathNavigationError(undefined);
          }}
          onKeyDown={(event) => {
            if (event.key !== "Enter") {
              return;
            }

            event.preventDefault();
            void navigateToPath(pathInputValue);
          }}
        />

        <button
          type="button"
          className="file-system-refresh-button"
          onClick={refresh}
          disabled={rootLoadState === "loading" || isPathNavigationPending}
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
