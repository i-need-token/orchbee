import type { TabPanelWorkspace } from './panel-workspace-state';
import type { TerminalTabPresentationHandoffState } from './transitionTerminalTabPresentationHandoff';
import type { TerminalTabPresentationLayer } from '../types/terminal-tab-presentation-layer';

interface ProjectedTerminalTabPresentationLayer {
	readonly epoch: number;
	readonly mode: TerminalTabPresentationLayer['mode'];
	readonly tab: TabPanelWorkspace;
}

/** Projects the current handoff state into keyed React layers ordered back to front. */
export function projectTerminalTabPresentationLayers(
	tabs: readonly TabPanelWorkspace[],
	state: TerminalTabPresentationHandoffState,
): readonly ProjectedTerminalTabPresentationLayer[] {
	const findTab = (tabId: string | null): TabPanelWorkspace | undefined =>
		tabId === null ? undefined : tabs.find((tab) => tab.tabId === tabId);
	if (state.kind === 'settled') {
		const tab = findTab(state.displayedTabId);
		return tab === undefined ? [] : [{ epoch: state.epoch, mode: 'active', tab }];
	}
	if (state.kind === 'activating') {
		const tab = findTab(state.displayedTabId);
		return tab === undefined ? [] : [{ epoch: state.epoch, mode: 'incoming', tab }];
	}
	const target = findTab(state.targetTabId);
	const displayed = findTab(state.displayedTabId);
	if (target === undefined) return [];
	if (displayed === undefined || displayed.tabId === target.tabId) {
		return [{ epoch: state.epoch, mode: 'incoming', tab: target }];
	}
	return [
		{ epoch: state.epoch, mode: 'incoming', tab: target },
		{ epoch: state.epoch, mode: 'retained', tab: displayed },
	];
}
