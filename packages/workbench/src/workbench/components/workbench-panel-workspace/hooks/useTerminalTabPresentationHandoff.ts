import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { TerminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalPresentationReadiness } from '../../../types/workbench-terminal-surface';
import type { TabPanelWorkspace } from '../model/panel-workspace-state';
import { projectTerminalTabPresentationHandoffTarget } from '../model/projectTerminalTabPresentationHandoffTarget';
import {
	transitionTerminalTabPresentationHandoff,
	type TerminalTabPresentationHandoffState,
} from '../model/transitionTerminalTabPresentationHandoff';

interface UseTerminalTabPresentationHandoffOptions {
	readonly activeTab: TabPanelWorkspace | null;
	readonly tabs: readonly TabPanelWorkspace[];
	readonly terminalSurfaceAvailable: boolean;
}

interface TerminalTabPresentationHandoffView {
	readonly displayedTab: TabPanelWorkspace | null;
	readonly state: TerminalTabPresentationHandoffState;
	reportPresentationReady(
		epoch: number,
		targetTabId: string,
		presentationId: TerminalPresentationId,
		readiness: WorkbenchTerminalPresentationReadiness,
	): void;
}

/** Coordinates one atomic visual and interaction handoff between active terminal Tabs. */
export function useTerminalTabPresentationHandoff({
	activeTab,
	tabs,
	terminalSurfaceAvailable,
}: UseTerminalTabPresentationHandoffOptions): TerminalTabPresentationHandoffView {
	const expectedPresentationIds = React.useMemo(
		() => projectTerminalTabPresentationHandoffTarget(activeTab, terminalSurfaceAvailable),
		[activeTab, terminalSurfaceAvailable],
	);
	const [presentation, setPresentation] = React.useState<{
		readonly displayedTab: TabPanelWorkspace | null;
		readonly state: TerminalTabPresentationHandoffState;
	}>(() => ({
		displayedTab: activeTab,
		state: transitionTerminalTabPresentationHandoff(null, {
			expectedPresentationIds,
			outgoingAvailable: false,
			targetTabId: activeTab?.tabId ?? null,
			type: 'target-observed',
		}),
	}));
	React.useLayoutEffect(() => {
		setPresentation((current) => {
			const currentDisplayedTab =
				current.state.displayedTabId === null
					? null
					: (tabs.find((tab) => tab.tabId === current.state.displayedTabId) ??
						(current.displayedTab?.tabId === current.state.displayedTabId
							? current.displayedTab
							: null));
			const state = transitionTerminalTabPresentationHandoff(current.state, {
				expectedPresentationIds,
				outgoingAvailable: currentDisplayedTab !== null,
				targetTabId: activeTab?.tabId ?? null,
				type: 'target-observed',
			});
			const displayedTab =
				state.displayedTabId === null
					? null
					: (tabs.find((tab) => tab.tabId === state.displayedTabId) ??
						(currentDisplayedTab?.tabId === state.displayedTabId ? currentDisplayedTab : null));
			return state === current.state && displayedTab === current.displayedTab
				? current
				: { displayedTab, state };
		});
	}, [activeTab?.tabId, expectedPresentationIds, tabs]);
	const reportPresentationReady = useMemoizedFn(
		(
			epoch: number,
			targetTabId: string,
			presentationId: TerminalPresentationId,
			readiness: WorkbenchTerminalPresentationReadiness,
		): void => {
			setPresentation((current) => {
				const state = transitionTerminalTabPresentationHandoff(current.state, {
					epoch,
					presentationId,
					readiness,
					targetTabId,
					type: 'presentation-ready',
				});
				const displayedTab =
					state.displayedTabId === null
						? null
						: (tabs.find((tab) => tab.tabId === state.displayedTabId) ??
							(current.displayedTab?.tabId === state.displayedTabId ? current.displayedTab : null));
				return state === current.state && displayedTab === current.displayedTab
					? current
					: { displayedTab, state };
			});
		},
	);

	return { ...presentation, reportPresentationReady };
}
