import {
	agentObservationMessages,
	type IntlShape,
	type MessageDescriptor,
	workbenchMessages,
} from '@orchbee/i18n';

import type { WorkbenchProjectWorkspaceRuntime } from '../../../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchAgentBoardRow,
	WorkbenchAgentBoardStatus,
} from '../../../types/workbench-agent-board';
import type { WorkbenchAgentState } from '../../../types/workbench-agent-observation';
import {
	formatWorkbenchAgentStatus,
	type WorkbenchAgentStatusView,
} from '../../../utils/formatWorkbenchAgentStatus';
import { formatWorkbenchAgentProvider } from '../../../utils/workbench-agent-names';
import {
	collectPanelLeaves,
	getTerminalDisplayTitle,
} from '../../workbench-panel-workspace/model/panel-workspace-state';

export interface WorkbenchAgentBoardRowView {
	/** The terminal, provider, freshness and place, as one line after the status. */
	readonly detail: string;
	readonly ended: boolean;
	readonly glyph: Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'>;
	readonly sessionId: string;
	readonly status: string;
}

/** What a row says about the terminal that names it. */
export interface WorkbenchPresentationNames {
	readonly tab: string;
	readonly title: string;
	readonly workspace: string;
}

/** The TUI's mapping from a session's status onto the shared glyph states. */
const GLYPH_STATES: Readonly<Record<WorkbenchAgentBoardStatus['kind'], WorkbenchAgentState>> = {
	canceled: 'error',
	ended: 'unknown',
	failed: 'error',
	idle: 'idle',
	interrupted: 'error',
	'needs-attention': 'blocked',
	succeeded: 'done',
	unknown: 'unknown',
	working: 'working',
};

const STATUS_MESSAGES: Readonly<
	Record<Exclude<WorkbenchAgentBoardStatus['kind'], 'needs-attention'>, MessageDescriptor>
> = {
	canceled: agentObservationMessages.statusCanceled,
	ended: agentObservationMessages.statusEnded,
	failed: agentObservationMessages.statusFailed,
	idle: agentObservationMessages.statusReady,
	interrupted: agentObservationMessages.statusInterrupted,
	succeeded: agentObservationMessages.statusSucceeded,
	unknown: agentObservationMessages.statusUnknown,
	working: agentObservationMessages.statusWorking,
};

const FRESHNESS_MESSAGES: Readonly<
	Record<Exclude<WorkbenchAgentBoardRow['freshness'], 'fresh'>, MessageDescriptor>
> = {
	disconnected: workbenchMessages.agentBoardDisconnected,
	stale: workbenchMessages.agentBoardStale,
};

const SEPARATOR = ' · ';

/**
 * Each row in the TUI's words: the status, then the terminal's name by the Tab strip's rule, the
 * provider, any freshness mark, and the Workspace with its Tab unless the Tab repeats the name. A
 * session no terminal of this window holds says so in place of the name and place.
 */
export function projectWorkbenchAgentBoardRows(
	intl: IntlShape,
	names: ReadonlyMap<string, WorkbenchPresentationNames>,
	rows: readonly WorkbenchAgentBoardRow[],
): readonly WorkbenchAgentBoardRowView[] {
	return rows.map((row) => {
		const place = row.presentationId === null ? undefined : names.get(row.presentationId);
		const glyph = formatWorkbenchAgentStatus(intl, GLYPH_STATES[row.status.kind]);
		return {
			detail: [
				place?.title.trim() ?? '',
				formatWorkbenchAgentProvider(intl, row.providerId),
				row.freshness === 'fresh' ? '' : intl.formatMessage(FRESHNESS_MESSAGES[row.freshness]),
				place === undefined
					? intl.formatMessage(workbenchMessages.agentBoardNoPanel)
					: formatPlace(place),
			]
				.filter((part) => part !== '')
				.join(SEPARATOR),
			ended: row.ended,
			glyph: { glyph: glyph.glyph, tone: glyph.tone },
			sessionId: row.sessionId,
			status:
				row.status.kind === 'needs-attention'
					? intl.formatMessage(agentObservationMessages.statusNeedsAttentionCount, {
							count: row.status.count,
						})
					: intl.formatMessage(STATUS_MESSAGES[row.status.kind]),
		};
	});
}

function formatPlace({ tab, title, workspace }: WorkbenchPresentationNames): string {
	const shownTab = tab.trim();
	if (shownTab === '' || shownTab === title.trim()) return workspace;
	return `${workspace}${SEPARATOR}${shownTab}`;
}

/** Every Workspace terminal's names, by Presentation. */
export function projectWorkbenchPresentationNames(
	runtime: WorkbenchProjectWorkspaceRuntime,
): ReadonlyMap<string, WorkbenchPresentationNames> {
	const names = new Map<string, WorkbenchPresentationNames>();
	for (const workspace of runtime.getSnapshot().workspaces) {
		const context = runtime.getTerminalContext(workspace.terminalContextId);
		if (context === null) continue;
		const panelTabs = new Map(
			context.panelWorkspaceStore.getSnapshot().tabs.map((tab) => [tab.tabId, tab] as const),
		);
		for (const tab of context.tabModel.getSnapshot().tabs) {
			const panelTab = panelTabs.get(tab.id);
			if (panelTab === undefined) continue;
			for (const panel of collectPanelLeaves(panelTab.root)) {
				for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
					names.set(terminal.presentationId, {
						tab: tab.title,
						title: getTerminalDisplayTitle(terminal),
						workspace: workspace.name,
					});
				}
			}
		}
	}
	return names;
}
