import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { TerminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalPresentationReadiness } from '../../../types/workbench-terminal-surface';
import type { TabPanelWorkspace } from '../model/panel-workspace-state';
import { projectTerminalTabPresentationHandoffTarget } from '../model/projectTerminalTabPresentationHandoffTarget';
import { projectTerminalTabPresentationRevision } from '../model/projectTerminalTabPresentationRevision';
import { resolveTerminalTabPresentationDisplayedTab } from '../model/resolveTerminalTabPresentationDisplayedTab';
import {
	transitionTerminalTabPresentationHandoff,
	type TerminalTabPresentationHandoffState,
} from '../model/transitionTerminalTabPresentationHandoff';

interface UseTerminalTabPresentationHandoffOptions {
	readonly activeTab: TabPanelWorkspace | null;
	readonly presentationScopeId: string;
	readonly tabs: readonly TabPanelWorkspace[];
	readonly terminalSurfaceAvailable: boolean;
}

interface TerminalTabPresentationHandoffView {
	readonly displayedTab: TabPanelWorkspace | null;
	readonly state: TerminalTabPresentationHandoffState;
	reportPresentationReady(
		epoch: number,
		targetTabId: string,
		targetRevision: string,
		presentationId: TerminalPresentationId,
		readiness: WorkbenchTerminalPresentationReadiness,
	): void;
}

/** Coordinates one atomic visual and interaction handoff between terminal layout revisions. */
export function useTerminalTabPresentationHandoff({
	activeTab,
	presentationScopeId,
	tabs,
	terminalSurfaceAvailable,
}: UseTerminalTabPresentationHandoffOptions): TerminalTabPresentationHandoffView {
	const targetRevision = React.useMemo(
		() => projectTerminalTabPresentationRevision(presentationScopeId, activeTab),
		[activeTab, presentationScopeId],
	);
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
			targetRevision,
			targetTabId: activeTab?.tabId ?? null,
			type: 'target-observed',
		}),
	}));

	React.useLayoutEffect(() => {
		setPresentation((current) => {
			const currentDisplayedTab = resolveTerminalTabPresentationDisplayedTab({
				activeTab,
				currentDisplayedTab: current.displayedTab,
				presentationScopeId,
				state: current.state,
				tabs,
				targetRevision,
			});
			const state = transitionTerminalTabPresentationHandoff(current.state, {
				expectedPresentationIds,
				outgoingAvailable: currentDisplayedTab !== null,
				targetRevision,
				targetTabId: activeTab?.tabId ?? null,
				type: 'target-observed',
			});
			const displayedTab = resolveTerminalTabPresentationDisplayedTab({
				activeTab,
				currentDisplayedTab,
				presentationScopeId,
				state,
				tabs,
				targetRevision,
			});
			return state === current.state && displayedTab === current.displayedTab
				? current
				: { displayedTab, state };
		});
	}, [activeTab, expectedPresentationIds, presentationScopeId, tabs, targetRevision]);

	const reportPresentationReady = useMemoizedFn(
		(
			epoch: number,
			targetTabId: string,
			reportedTargetRevision: string,
			presentationId: TerminalPresentationId,
			readiness: WorkbenchTerminalPresentationReadiness,
		): void => {
			setPresentation((current) => {
				const state = transitionTerminalTabPresentationHandoff(current.state, {
					epoch,
					presentationId,
					readiness,
					targetRevision: reportedTargetRevision,
					targetTabId,
					type: 'presentation-ready',
				});
				const displayedTab = resolveTerminalTabPresentationDisplayedTab({
					activeTab,
					currentDisplayedTab: current.displayedTab,
					presentationScopeId,
					state,
					tabs,
					targetRevision,
				});
				return state === current.state && displayedTab === current.displayedTab
					? current
					: { displayedTab, state };
			});
		},
	);

	return { ...presentation, reportPresentationReady };
}
