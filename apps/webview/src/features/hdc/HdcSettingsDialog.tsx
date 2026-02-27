import type { BinConfigSource } from "@harmony/protocol";
import { useEffect, useState } from "react";

interface HdcSettingsDialogProps {
  open: boolean;
  loading: boolean;
  saving: boolean;
  customBinPath: string | null;
  resolvedBinPath: string | null;
  source: BinConfigSource;
  message?: string;
  onClose: () => void;
  onSave: (path: string) => Promise<void>;
  onClear: () => Promise<void>;
}

function sourceLabel(source: BinConfigSource): string {
  if (source === "custom") {
    return "custom";
  }

  if (source === "path") {
    return "PATH";
  }

  if (source === "deveco") {
    return "DevEco default";
  }

  return "none";
}

export function HdcSettingsDialog({
  open,
  loading,
  saving,
  customBinPath,
  resolvedBinPath,
  source,
  message,
  onClose,
  onSave,
  onClear
}: HdcSettingsDialogProps) {
  const [inputPath, setInputPath] = useState("");
  const [localError, setLocalError] = useState<string>();

  useEffect(() => {
    if (open) {
      setInputPath(customBinPath ?? "");
      setLocalError(undefined);
    }
  }, [open, customBinPath]);

  if (!open) {
    return null;
  }

  return (
    <div className="settings-overlay" role="presentation" onClick={onClose}>
      <div
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="HDC Settings"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className="settings-header">
          <h2>HDC Settings</h2>
          <button type="button" className="settings-close" onClick={onClose} aria-label="Close settings">
            X
          </button>
        </div>

        <label htmlFor="hdc-bin-path" className="settings-label">
          Custom HDC binary path
        </label>
        <input
          id="hdc-bin-path"
          className="settings-input"
          type="text"
          value={inputPath}
          placeholder="/path/to/hdc"
          onChange={(event) => {
            setInputPath(event.target.value);
            setLocalError(undefined);
          }}
        />

        <div className="settings-actions">
          <button
            type="button"
            className="settings-primary"
            disabled={saving || loading}
            onClick={async () => {
              try {
                await onSave(inputPath.trim());
              } catch (error) {
                setLocalError(error instanceof Error ? error.message : String(error));
              }
            }}
          >
            {saving ? "Saving..." : "Save"}
          </button>

          <button
            type="button"
            className="settings-secondary"
            disabled={saving || loading}
            onClick={async () => {
              try {
                await onClear();
                setInputPath("");
              } catch (error) {
                setLocalError(error instanceof Error ? error.message : String(error));
              }
            }}
          >
            Clear custom path
          </button>
        </div>

        <div className="settings-meta">
          <p>
            <strong>Detected source:</strong> {sourceLabel(source)}
          </p>
          <p>
            <strong>Resolved binary:</strong> {resolvedBinPath ?? "not available"}
          </p>
        </div>

        {message ? <p className="settings-message settings-message-warning">{message}</p> : null}
        {localError ? <p className="settings-message settings-message-error">{localError}</p> : null}
      </div>
    </div>
  );
}
