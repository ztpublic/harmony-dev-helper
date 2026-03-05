import type { BinConfigSource } from "@harmony/protocol";
import { useEffect, useState } from "react";
import {
  MAX_HILOG_HISTORY_LIMIT,
  MIN_HILOG_HISTORY_LIMIT,
  type AppTheme,
  normalizeHilogHistoryLimit
} from "./appSettings";

interface SettingsDialogProps {
  open: boolean;
  loading: boolean;
  saving: boolean;
  canBrowseHdcPath: boolean;
  customBinPath: string | null;
  resolvedBinPath: string | null;
  source: BinConfigSource;
  message?: string;
  hilogHistoryLimit: number;
  theme: AppTheme;
  onClose: () => void;
  onBrowseHdcPath: () => Promise<string | null>;
  onSaveHdcPath: (path: string) => Promise<void>;
  onClearHdcPath: () => Promise<void>;
  onSaveHilogHistoryLimit: (limit: number) => void;
  onSaveTheme: (theme: AppTheme) => void;
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

function parseHistoryLimit(input: string): number {
  const parsed = Number.parseInt(input, 10);
  return normalizeHilogHistoryLimit(parsed);
}

export function SettingsDialog({
  open,
  loading,
  saving,
  canBrowseHdcPath,
  customBinPath,
  resolvedBinPath,
  source,
  message,
  hilogHistoryLimit,
  theme,
  onClose,
  onBrowseHdcPath,
  onSaveHdcPath,
  onClearHdcPath,
  onSaveHilogHistoryLimit,
  onSaveTheme
}: SettingsDialogProps) {
  const [inputPath, setInputPath] = useState("");
  const [browsingPath, setBrowsingPath] = useState(false);
  const [localError, setLocalError] = useState<string>();
  const [historyInput, setHistoryInput] = useState(String(hilogHistoryLimit));
  const [historyError, setHistoryError] = useState<string>();
  const [historySaved, setHistorySaved] = useState<string>();

  useEffect(() => {
    if (open) {
      setInputPath(customBinPath ?? "");
      setBrowsingPath(false);
      setLocalError(undefined);
      setHistoryInput(String(hilogHistoryLimit));
      setHistoryError(undefined);
      setHistorySaved(undefined);
    }
  }, [open, customBinPath, hilogHistoryLimit]);

  if (!open) {
    return null;
  }

  return (
    <div className="settings-overlay" role="presentation" onClick={onClose}>
      <div
        className="settings-dialog"
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <div className="settings-header">
          <h2>Settings</h2>
          <button type="button" className="settings-close" onClick={onClose} aria-label="Close settings">
            X
          </button>
        </div>

        <section className="settings-section">
          <h3 className="settings-section-title">Appearance</h3>
          <fieldset className="settings-fieldset">
            <legend className="settings-label">Theme</legend>

            <div className="settings-theme-options" role="radiogroup" aria-label="Theme">
              <label className="settings-radio-option">
                <input
                  type="radio"
                  name="settings-theme"
                  value="dark"
                  checked={theme === "dark"}
                  onChange={() => {
                    onSaveTheme("dark");
                  }}
                />
                <span>Dark</span>
              </label>

              <label className="settings-radio-option">
                <input
                  type="radio"
                  name="settings-theme"
                  value="light"
                  checked={theme === "light"}
                  onChange={() => {
                    onSaveTheme("light");
                  }}
                />
                <span>Light</span>
              </label>
            </div>
          </fieldset>

          <p className="settings-hint">Theme applies immediately.</p>
        </section>

        <section className="settings-section">
          <h3 className="settings-section-title">HDC</h3>
          <label htmlFor="hdc-bin-path" className="settings-label">
            Custom HDC binary path
          </label>
          <div className="settings-input-row">
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
            {canBrowseHdcPath ? (
              <button
                type="button"
                className="settings-secondary"
                disabled={saving || loading || browsingPath}
                onClick={async () => {
                  setBrowsingPath(true);
                  setLocalError(undefined);

                  try {
                    const selectedPath = await onBrowseHdcPath();
                    if (selectedPath) {
                      setInputPath(selectedPath);
                    }
                  } catch (error) {
                    setLocalError(error instanceof Error ? error.message : String(error));
                  } finally {
                    setBrowsingPath(false);
                  }
                }}
              >
                {browsingPath ? "Browsing..." : "Browse"}
              </button>
            ) : null}
          </div>

          <div className="settings-actions">
            <button
              type="button"
              className="settings-primary"
              disabled={saving || loading}
              onClick={async () => {
                try {
                  await onSaveHdcPath(inputPath.trim());
                } catch (error) {
                  setLocalError(error instanceof Error ? error.message : String(error));
                }
              }}
            >
              {saving ? "Saving..." : "Save HDC path"}
            </button>

            <button
              type="button"
              className="settings-secondary"
              disabled={saving || loading}
              onClick={async () => {
                try {
                  await onClearHdcPath();
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
        </section>

        <section className="settings-section">
          <h3 className="settings-section-title">Hilog</h3>
          <label htmlFor="hilog-history-limit" className="settings-label">
            History limit (lines)
          </label>
          <input
            id="hilog-history-limit"
            className="settings-input"
            type="number"
            min={MIN_HILOG_HISTORY_LIMIT}
            max={MAX_HILOG_HISTORY_LIMIT}
            step={1000}
            value={historyInput}
            onChange={(event) => {
              setHistoryInput(event.target.value);
              setHistoryError(undefined);
              setHistorySaved(undefined);
            }}
          />

          <p className="settings-hint">
            Allowed range: {MIN_HILOG_HISTORY_LIMIT} - {MAX_HILOG_HISTORY_LIMIT}. Default: 10000.
          </p>

          <div className="settings-actions">
            <button
              type="button"
              className="settings-primary"
              onClick={() => {
                const parsedRaw = Number.parseInt(historyInput, 10);
                if (!Number.isFinite(parsedRaw)) {
                  setHistoryError("History limit must be a number.");
                  return;
                }

                if (parsedRaw < MIN_HILOG_HISTORY_LIMIT || parsedRaw > MAX_HILOG_HISTORY_LIMIT) {
                  setHistoryError(
                    `History limit must be between ${MIN_HILOG_HISTORY_LIMIT} and ${MAX_HILOG_HISTORY_LIMIT}.`
                  );
                  return;
                }

                const normalized = parseHistoryLimit(historyInput);
                onSaveHilogHistoryLimit(normalized);
                setHistoryInput(String(normalized));
                setHistoryError(undefined);
                setHistorySaved(`Saved: ${normalized} lines`);
              }}
            >
              Save Hilog settings
            </button>
          </div>

          {historySaved ? <p className="settings-message settings-message-success">{historySaved}</p> : null}
          {historyError ? <p className="settings-message settings-message-error">{historyError}</p> : null}
        </section>
      </div>
    </div>
  );
}
