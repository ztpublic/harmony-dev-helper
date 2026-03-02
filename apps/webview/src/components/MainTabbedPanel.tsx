import type { ReactNode } from "react";
import type { MainPanelTabDefinition, MainPanelTabId } from "../features/mainPanel/mainPanelTabs";

interface MainTabbedPanelProps {
  tabs: readonly MainPanelTabDefinition[];
  activeTabId: MainPanelTabId;
  onTabChange: (tabId: MainPanelTabId) => void;
  panels: Record<MainPanelTabId, ReactNode>;
  headerRight?: ReactNode;
}

export function MainTabbedPanel({
  tabs,
  activeTabId,
  onTabChange,
  panels,
  headerRight
}: MainTabbedPanelProps) {
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
  const activePanelId = `main-tabpanel-${activeTab.id}`;
  const activeTabButtonId = `main-tab-${activeTab.id}`;

  return (
    <section className="main-tabbed-panel" aria-label="Main panel">
      <div className="main-tabbed-header">
        <div className="main-tabs" role="tablist" aria-label="Main feature tabs">
          {tabs.map((tab) => {
            const isActive = tab.id === activeTab.id;
            const tabId = `main-tab-${tab.id}`;
            const panelId = `main-tabpanel-${tab.id}`;

            return (
              <button
                key={tab.id}
                id={tabId}
                type="button"
                role="tab"
                className="main-tab-button"
                aria-selected={isActive}
                aria-controls={panelId}
                tabIndex={isActive ? 0 : -1}
                onClick={() => {
                  onTabChange(tab.id);
                }}
              >
                {tab.label}
              </button>
            );
          })}
        </div>

        {headerRight ? <div className="main-tabbed-header-right">{headerRight}</div> : null}
      </div>

      <div id={activePanelId} className="main-tab-panel" role="tabpanel" aria-labelledby={activeTabButtonId}>
        {panels[activeTab.id]}
      </div>
    </section>
  );
}
