import type { TabPanelWorkspace } from './panel-workspace-state';
import type { TerminalTabPresentationHandoffState } from './transitionTerminalTabPresentationHandoff';
import type { TerminalTabPresentationLayer } from '../types/terminal-tab-presentation-layer';

interface ProjectedTerminalTabPresentationLayer {
	readonly epoch: number;
	readonly mode: TerminalTabPresentationLayer['mode'];
	readonly revision: string;
	readonly tab: TabPanelWorkspace;
}

/** Projects the current handoff state into keyed React layers ordered back to front. */
export function projectTerminalTabPresentationLayers(
	targetTab: TabPanelWorkspace | null,
	state: TerminalTabPresentationHandoffState,
	displayedTab: TabPanelWorkspace | null,
): readonly ProjectedTerminalTabPresentationLayer[] {
	if (state.kind === 'settled') {
		return displayedTab === null || state.displayedRevision === null
			? []
			: [
					{
						epoch: state.epoch,
						mode: 'active',
						revision: state.displayedRevision,
						tab: displayedTab,
					},
				];
	}
	if (state.kind === 'activating') {
		return targetTab === null
			? []
			: [
					{
						epoch: state.epoch,
						mode: 'incoming',
						revision: state.targetRevision,
						tab: targetTab,
					},
				];
	}
	if (targetTab === null) return [];
	const target = {
		epoch: state.epoch,
		mode: 'incoming' as const,
		revision: state.targetRevision,
		tab: targetTab,
	};
	if (displayedTab === null || state.displayedRevision === null) return [target];
	if (state.displayedRevision === state.targetRevision) {
		return [target];
	}
	return [
		target,
		{
			epoch: state.epoch,
			mode: 'retained',
			revision: state.displayedRevision,
			tab: displayedTab,
		},
	];
}
