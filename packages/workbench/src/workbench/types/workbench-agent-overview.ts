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

export interface WorkbenchWorkspaceAgentOverviewView {
	readonly agentLabel: string;
	/** Distinct reported Agent labels in row order. */
	readonly agents: readonly string[];
	readonly collapseLabel: string;
	readonly countLabel: string;
	readonly expandLabel: string;
	readonly projectId: string | null;
	readonly rows: readonly WorkbenchWorkspaceAgentRowView[];
	/** The most urgent row state. */
	readonly status: WorkbenchAgentStatusView;
	readonly unreadLabel?: string | undefined;
	readonly visibleCountLabel: string;
	readonly workspaceId: string;
}
