import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchAgentStatusView } from '../utils/formatWorkbenchAgentStatus';

interface WorkbenchWorkspaceAgentRowView {
	readonly accessibleLabel: string;
	/** The Agent's product name, or a custom report's own name. */
	readonly agentLabel: string;
	readonly agents: readonly string[];
	readonly locationLabel: string;
	readonly presentationId: TerminalPresentationId;
	readonly status: WorkbenchAgentStatusView;
	readonly unreadLabel?: string | undefined;
}

/** A folded Workspace's second line: its Agents' states, most urgent first. */
interface WorkbenchWorkspaceAgentFoldedStatusView {
	/** Each shown Agent's state, keyed by its terminal Presentation. */
	readonly glyphs: readonly (Pick<WorkbenchAgentStatusView, 'glyph' | 'tone'> & {
		readonly presentationId: TerminalPresentationId;
	})[];
	/** Each distinct state in words, in the order shown. */
	readonly label: string;
	/** States past the shown glyphs, counted as `+N`; zero shows none. */
	readonly remainder: number;
}

export interface WorkbenchWorkspaceAgentOverviewView {
	readonly agentLabel: string;
	/** Distinct reported Agent labels in row order. */
	readonly agents: readonly string[];
	readonly collapseLabel: string;
	readonly countLabel: string;
	readonly expandLabel: string;
	/** Whether the Agent rows show beneath the Workspace; otherwise its second line lists their states. */
	readonly expanded: boolean;
	readonly foldedStatus: WorkbenchWorkspaceAgentFoldedStatusView;
	readonly projectId: string | null;
	readonly rows: readonly WorkbenchWorkspaceAgentRowView[];
	/** The most urgent row state. */
	readonly status: WorkbenchAgentStatusView;
	readonly unreadLabel?: string | undefined;
	readonly visibleCountLabel: string;
	readonly workspaceId: string;
}
