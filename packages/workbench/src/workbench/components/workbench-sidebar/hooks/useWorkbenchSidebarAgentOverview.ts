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
		readonly projectId: string;
		readonly workspaceId: string;
	} | null;
	readonly toggleWorkspace: (projectId: string, workspaceId: string) => void;
}

export function useWorkbenchSidebarAgentOverview({
	expandedProjectIds,
	workspaces,
}: UseWorkbenchSidebarAgentOverviewOptions): WorkbenchSidebarAgentOverviewView {
	const agentGradientPrefix = React.useId().replaceAll(':', '');
	const [expandedWorkspace, setExpandedWorkspace] = React.useState<{
		readonly projectId: string;
		readonly workspaceId: string;
	} | null>(null);
	React.useEffect(() => {
		if (expandedWorkspace === null) return;
		const projectExpanded = expandedProjectIds.includes(expandedWorkspace.projectId);
		const overviewExists = workspaces.some(
			(workspace) =>
				workspace.projectId === expandedWorkspace.projectId &&
				workspace.workspaceId === expandedWorkspace.workspaceId,
		);
		if (!projectExpanded || !overviewExists) setExpandedWorkspace(null);
	}, [expandedProjectIds, expandedWorkspace, workspaces]);
	const toggleWorkspace = useMemoizedFn((projectId: string, workspaceId: string): void => {
		setExpandedWorkspace((current) =>
			current?.projectId === projectId && current.workspaceId === workspaceId
				? null
				: { projectId, workspaceId },
		);
	});
	return { agentGradientPrefix, expandedWorkspace, toggleWorkspace };
}
