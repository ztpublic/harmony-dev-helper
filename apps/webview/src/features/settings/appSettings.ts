import type { HarmonyHost } from "@harmony/protocol";

export interface AppSettings {
  hilogHistoryLimit: number;
}

export const DEFAULT_HILOG_HISTORY_LIMIT = 10_000;
export const MIN_HILOG_HISTORY_LIMIT = 1_000;
export const MAX_HILOG_HISTORY_LIMIT = 100_000;

const STORAGE_KEY_PREFIX = "harmony.settings.v1";

function storageKey(host: HarmonyHost): string {
  return `${STORAGE_KEY_PREFIX}.${host}`;
}

export function normalizeHilogHistoryLimit(value: number): number {
  if (!Number.isFinite(value)) {
    return DEFAULT_HILOG_HISTORY_LIMIT;
  }

  return Math.min(MAX_HILOG_HISTORY_LIMIT, Math.max(MIN_HILOG_HISTORY_LIMIT, Math.round(value)));
}

export function readAppSettings(host: HarmonyHost): AppSettings {
  if (typeof window === "undefined") {
    return { hilogHistoryLimit: DEFAULT_HILOG_HISTORY_LIMIT };
  }

  try {
    const raw = window.localStorage.getItem(storageKey(host));
    if (!raw) {
      return { hilogHistoryLimit: DEFAULT_HILOG_HISTORY_LIMIT };
    }

    const parsed = JSON.parse(raw) as Partial<AppSettings>;
    return {
      hilogHistoryLimit: normalizeHilogHistoryLimit(parsed.hilogHistoryLimit ?? DEFAULT_HILOG_HISTORY_LIMIT)
    };
  } catch {
    return { hilogHistoryLimit: DEFAULT_HILOG_HISTORY_LIMIT };
  }
}

export function persistAppSettings(host: HarmonyHost, settings: AppSettings): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    const normalized: AppSettings = {
      hilogHistoryLimit: normalizeHilogHistoryLimit(settings.hilogHistoryLimit)
    };
    window.localStorage.setItem(storageKey(host), JSON.stringify(normalized));
  } catch {
    // Ignore persistence failures to avoid blocking core features.
  }
}
