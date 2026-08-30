import type { AgentProviderId, AgentSessionId } from '@orchbee/agent';

import type { WorkbenchAgentStatusView } from '../utils/formatWorkbenchAgentStatus';

interface WorkbenchWorkspaceAgentRowView {
	readonly accessibleLabel: string;
	readonly locationLabel: string;
	readonly navigable: boolean;
	readonly providerIds: readonly AgentProviderId[];
	readonly providerLabel: string;
	readonly sessionId: AgentSessionId;
	readonly status: WorkbenchAgentStatusView;
}

export interface WorkbenchWorkspaceAgentOverviewView {
	readonly collapseLabel: string;
	readonly countLabel: string;
	readonly expandLabel: string;
	readonly projectId: string;
	readonly providerIds: readonly AgentProviderId[];
	readonly rows: readonly WorkbenchWorkspaceAgentRowView[];
	readonly sessionCount: number;
	readonly status: WorkbenchAgentStatusView;
	readonly visibleCountLabel: string;
	readonly workspaceId: string;
}
