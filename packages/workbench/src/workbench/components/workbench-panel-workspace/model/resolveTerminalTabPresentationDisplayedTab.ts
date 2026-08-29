import type { TabPanelWorkspace } from './panel-workspace-state';
import { projectTerminalTabPresentationRevision } from './projectTerminalTabPresentationRevision';
import type { TerminalTabPresentationHandoffState } from './transitionTerminalTabPresentationHandoff';

/** Resolves the exact immutable Tab snapshot named by the displayed handoff revision. */
export function resolveTerminalTabPresentationDisplayedTab({
	activeTab,
	currentDisplayedTab,
	presentationScopeId,
	state,
	tabs,
	targetRevision,
}: {
	readonly activeTab: TabPanelWorkspace | null;
	readonly currentDisplayedTab: TabPanelWorkspace | null;
	readonly presentationScopeId: string;
	readonly state: TerminalTabPresentationHandoffState;
	readonly tabs: readonly TabPanelWorkspace[];
	readonly targetRevision: string | null;
}): TabPanelWorkspace | null {
	if (state.displayedTabId === null || state.displayedRevision === null) return null;
	if (activeTab?.tabId === state.displayedTabId && targetRevision === state.displayedRevision) {
		return activeTab;
	}
	const canonical = tabs.find(
		(tab) =>
			tab.tabId === state.displayedTabId &&
			projectTerminalTabPresentationRevision(presentationScopeId, tab) === state.displayedRevision,
	);
	if (canonical !== undefined) return canonical;
	return currentDisplayedTab?.tabId === state.displayedTabId ? currentDisplayedTab : null;
}
