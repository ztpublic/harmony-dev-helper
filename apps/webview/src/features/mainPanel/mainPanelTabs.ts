import type { HarmonyHost } from "@harmony/protocol";

export const MAIN_PANEL_TABS = [{ id: "hilog", label: "Hilog" }] as const;

export type MainPanelTabId = (typeof MAIN_PANEL_TABS)[number]["id"];

export interface MainPanelTabDefinition {
  id: MainPanelTabId;
  label: string;
}

export const DEFAULT_MAIN_PANEL_TAB_ID: MainPanelTabId = MAIN_PANEL_TABS[0].id;

const MAIN_PANEL_TAB_IDS = new Set<MainPanelTabId>(MAIN_PANEL_TABS.map((tab) => tab.id));
const STORAGE_KEY_PREFIX = "harmony.mainPanel.activeTab.v1";

function storageKey(host: HarmonyHost): string {
  return `${STORAGE_KEY_PREFIX}.${host}`;
}

function isMainPanelTabId(value: string): value is MainPanelTabId {
  return MAIN_PANEL_TAB_IDS.has(value as MainPanelTabId);
}

export function readPersistedMainTab(host: HarmonyHost, fallbackTabId: MainPanelTabId): MainPanelTabId {
  if (typeof window === "undefined") {
    return fallbackTabId;
  }

  try {
    const persistedTab = window.localStorage.getItem(storageKey(host));
    if (!persistedTab || !isMainPanelTabId(persistedTab)) {
      return fallbackTabId;
    }

    return persistedTab;
  } catch {
    return fallbackTabId;
  }
}

export function persistMainTab(host: HarmonyHost, tabId: MainPanelTabId): void {
  if (typeof window === "undefined") {
    return;
  }

  try {
    window.localStorage.setItem(storageKey(host), tabId);
  } catch {
    // Ignore storage failures so tab switching still works.
  }
}
