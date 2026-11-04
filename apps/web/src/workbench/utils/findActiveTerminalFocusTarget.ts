import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';

export function findActiveTerminalFocusTarget(
	projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime,
): HTMLElement | null {
	if (typeof document === 'undefined') return null;
	const terminalContext = projectWorkspaceRuntime.getActiveTerminalContext();
	const activeTabId = terminalContext?.tabModel.getSnapshot().activeTabId ?? null;
	if (terminalContext === null || activeTabId === null) return null;
	const activeTab = terminalContext.panelWorkspaceStore
		.getSnapshot()
		.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) return null;
	const viewport =
		Array.from(
			document.querySelectorAll<HTMLElement>('[data-slot="workbench-terminal-viewport"]'),
		).find((element) => element.dataset['panelId'] === activeTab.focusedPanelId) ?? null;
	return viewport?.querySelector<HTMLElement>('textarea, [role="textbox"]') ?? viewport;
}
