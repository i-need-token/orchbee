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
	const [state, setState] = React.useState<TerminalTabPresentationHandoffState>(() =>
		transitionTerminalTabPresentationHandoff(null, {
			expectedPresentationIds,
			outgoingAvailable: false,
			targetTabId: activeTab?.tabId ?? null,
			type: 'target-observed',
		}),
	);
	React.useLayoutEffect(() => {
		setState((current) =>
			transitionTerminalTabPresentationHandoff(current, {
				expectedPresentationIds,
				outgoingAvailable:
					current.displayedTabId !== null &&
					tabs.some((tab) => tab.tabId === current.displayedTabId),
				targetTabId: activeTab?.tabId ?? null,
				type: 'target-observed',
			}),
		);
	}, [activeTab?.tabId, expectedPresentationIds, tabs]);
	const reportPresentationReady = useMemoizedFn(
		(
			epoch: number,
			targetTabId: string,
			presentationId: TerminalPresentationId,
			readiness: WorkbenchTerminalPresentationReadiness,
		): void => {
			setState((current) =>
				transitionTerminalTabPresentationHandoff(current, {
					epoch,
					presentationId,
					readiness,
					targetTabId,
					type: 'presentation-ready',
				}),
			);
		},
	);

	return { reportPresentationReady, state };
}
