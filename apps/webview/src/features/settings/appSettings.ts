import type { HarmonyHost } from "@harmony/protocol";

export type AppTheme = "dark" | "light";

export interface AppSettings {
  hilogHistoryLimit: number;
  theme: AppTheme;
}

export const DEFAULT_HILOG_HISTORY_LIMIT = 10_000;
export const MIN_HILOG_HISTORY_LIMIT = 1_000;
export const MAX_HILOG_HISTORY_LIMIT = 100_000;
export const DEFAULT_APP_THEME: AppTheme = "dark";

const STORAGE_KEY_PREFIX = "harmony.settings.v1";

function storageKey(host: HarmonyHost): string {
  return `${STORAGE_KEY_PREFIX}.${host}`;
}

function defaultAppSettings(): AppSettings {
  return {
    hilogHistoryLimit: DEFAULT_HILOG_HISTORY_LIMIT,
    theme: DEFAULT_APP_THEME
  };
}

export function normalizeHilogHistoryLimit(value: number): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_HILOG_HISTORY_LIMIT;
  }

  return Math.min(MAX_HILOG_HISTORY_LIMIT, Math.max(MIN_HILOG_HISTORY_LIMIT, Math.round(value)));
}

export function normalizeAppTheme(value: unknown): AppTheme {
  if (value === "light" || value === "dark") {
    return value;
  }

  return DEFAULT_APP_THEME;
}

export function readAppSettings(host: HarmonyHost): AppSettings {
  if (typeof window === "undefined") {
    return defaultAppSettings();
  }

  try {
    const raw = window.localStorage.getItem(storageKey(host));
    if (!raw) {
      return defaultAppSettings();
    }

    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return {
      hilogHistoryLimit: normalizeHilogHistoryLimit(parsed.hilogHistoryLimit ?? DEFAULT_HILOG_HISTORY_LIMIT),
      theme: normalizeAppTheme(parsed.theme)
    };
  } catch {
    return defaultAppSettings();
  }
}

export function persistAppSettings(host: HarmonyHost, settings: AppSettings): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    const normalized: AppSettings = {
      hilogHistoryLimit: normalizeHilogHistoryLimit(settings.hilogHistoryLimit),
      theme: normalizeAppTheme(settings.theme)
    };
    window.localStorage.setItem(storageKey(host), JSON.stringify(normalized));
  } catch {
    // Ignore persistence failures to avoid blocking core features.
  }
}
