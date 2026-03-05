import { useCallback, useEffect, useRef, useState } from "react";
import {
  Tree,
  type NodeApi,
  type NodeRendererProps,
  type RowRendererProps,
  type TreeApi
} from "react-arborist";
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

const ROOT_PATH_DEFAULT = "/";
const ROOT_REQUEST_KEY = "__root__";
const TREE_HEIGHT_DEFAULT = 360;
const TREE_ROW_HEIGHT = 26;
const TREE_INDENT = 16;

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

  const treeApiRef = useRef<TreeApi<FileSystemNode> | null>(null);
  const treeDataRef = useRef<readonly FileSystemNode[]>([]);
  const rootLoadStateRef = useRef<DirectoryLoadState>("unloaded");
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
    onSelectionChangeRef.current = onSelectionChange;
  }, [onSelectionChange]);

  useEffect(() => {
    onOpenFileRef.current = onOpenFile;
  }, [onOpenFile]);

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
        const entries = await vfs.listDirectory(rootPath);

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
    [rootPath, vfs]
  );

  const refresh = useCallback(() => {
    const nextVersion = cacheVersionRef.current + 1;
    cacheVersionRef.current = nextVersion;
    inFlightRef.current.clear();
    setTreeData([]);
    setRootLoadState("unloaded");
    setRootErrorMessage(undefined);
    onSelectionChangeRef.current?.(null);
    void loadRootDirectory({
      force: true,
      version: nextVersion
    });
  }, [loadRootDirectory]);

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

  const handleSelect = useCallback((nodes: NodeApi<FileSystemNode>[]) => {
    const firstSelected = nodes[0];
    onSelectionChangeRef.current?.(firstSelected ? toPublicEntry(firstSelected.data) : null);
  }, []);

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
            <button
              type="button"
              className="file-system-expander"
              aria-label={node.isOpen ? `Collapse ${entry.name}` : `Expand ${entry.name}`}
              onClick={(event) => {
                event.stopPropagation();
                node.toggle();
              }}
            >
              <svg
                className={`file-system-expander-icon ${node.isOpen ? "file-system-expander-icon-open" : ""}`}
                viewBox="0 0 16 16"
                aria-hidden="true"
                focusable="false"
              >
                <path d="M6 4.5L10 8L6 11.5" />
              </svg>
            </button>
          ) : (
            <span className="file-system-expander-placeholder" aria-hidden="true" />
          )}

          <span
            className={`file-system-kind-dot ${
              isDirectory ? "file-system-kind-dot-directory" : "file-system-kind-dot-file"
            }`}
            aria-hidden="true"
          />

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

  const rootSummary = rootPath === "/" ? "Root: /" : `Root: ${rootPath}`;

  return (
    <section className="panel file-system-panel" aria-label="File system">
      <div className="file-system-header">
        <div>
          <p className="kicker">Files</p>
          <h2>File System</h2>
        </div>

        <button
          type="button"
          className="file-system-refresh-button"
          onClick={refresh}
          disabled={rootLoadState === "loading"}
        >
          {rootLoadState === "loading" ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      <p className="file-system-root-path" title={rootPath}>
        {rootSummary}
      </p>

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
    </section>
  );
}
