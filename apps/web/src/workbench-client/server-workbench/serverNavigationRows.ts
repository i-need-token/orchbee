import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import { inDisplayOrder } from './inDisplayOrder';

/**
 * A Session's Projects and Workspaces in display order, as another Session's sidebar lists them:
 * names and membership only.
 */
export function serverNavigationRows(snapshot: WorkbenchSnapshot): {
	readonly projects: readonly { readonly id: string; readonly name: string }[];
	readonly workspaces: readonly {
		readonly id: string;
		readonly name: string;
		readonly projectId: string | null;
	}[];
} {
	const { navigation } = snapshot;
	return {
		projects: inDisplayOrder(
			navigation.projects,
			navigation.projectOrder,
			(project) => project.projectId,
		).map((project) => ({ id: project.projectId, name: project.label })),
		workspaces: inDisplayOrder(
			navigation.workspaces,
			navigation.workspaceOrder,
			(workspace) => workspace.workspaceId,
		).map((workspace) => ({
			id: workspace.workspaceId,
			name: workspace.label,
			projectId: workspace.projectId,
		})),
	};
}
