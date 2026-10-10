import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchWorkspaceAgentOverviewView } from '../../../types/workbench-agent-overview';

interface UseWorkbenchSidebarAgentOverviewOptions {
	readonly expandedProjectIds: readonly string[];
	readonly workspaces: readonly Pick<
		WorkbenchWorkspaceAgentOverviewView,
		'projectId' | 'workspaceId'
	>[];
}

interface WorkbenchSidebarAgentOverviewView {
	readonly agentGradientPrefix: string;
	readonly expandedWorkspace: {
		readonly workspaceId: string;
	} | null;
	readonly toggleWorkspace: (workspaceId: string) => void;
}

export function useWorkbenchSidebarAgentOverview({
	workspaces,
}: UseWorkbenchSidebarAgentOverviewOptions): WorkbenchSidebarAgentOverviewView {
	const agentGradientPrefix = React.useId().replaceAll(':', '');
	const [expandedWorkspace, setExpandedWorkspace] = React.useState<{
		readonly workspaceId: string;
	} | null>(null);
	React.useEffect(() => {
		if (expandedWorkspace === null) return;

		const overviewExists = workspaces.some(
			(workspace) => workspace.workspaceId === expandedWorkspace.workspaceId,
		);
		if (!overviewExists) setExpandedWorkspace(null);
	}, [expandedWorkspace, workspaces]);
	const toggleWorkspace = useMemoizedFn((workspaceId: string): void => {
		setExpandedWorkspace((current) =>
			current?.workspaceId === workspaceId ? null : { workspaceId },
		);
	});
	return { agentGradientPrefix, expandedWorkspace, toggleWorkspace };
}
