import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	type WorkbenchClientView,
	type WorkbenchPublication,
} from '../../workbench';
import { createFocusRecoveredWorkbenchClientView } from './workbench-focus-recovery-storage';

/** Where a window relaunched by Update opens: the terminal it displayed, or the Workspace it showed. */
export type WorkbenchRelaunchPlace =
	| { readonly kind: 'terminal'; readonly presentationId: TerminalPresentationId }
	| { readonly kind: 'workspace'; readonly workspaceId: string };

/**
 * Opens a relaunched window at its place: a terminal as renderer reload recovery projects one, with
 * its Workspace, exact Tab and focused Panel, or a Workspace by itself. A place the publication no
 * longer holds changes nothing.
 */
export function createRelaunchPlaceWorkbenchClientView(
	publication: WorkbenchPublication,
	place: WorkbenchRelaunchPlace,
): WorkbenchClientView | undefined {
	if (place.kind === 'terminal') {
		return createFocusRecoveredWorkbenchClientView(publication, place.presentationId);
	}
	const workspace = publication.workspaces.find((candidate) => candidate.id === place.workspaceId);
	if (workspace === undefined) return undefined;
	const defaultView = createDefaultWorkbenchClientView(publication);
	return createWorkbenchClientView({
		activeLocation: { kind: 'workspace', workspaceId: workspace.id },
		expandedProjectIds:
			workspace.projectId === null
				? defaultView.expandedProjectIds
				: [...new Set([...defaultView.expandedProjectIds, workspace.projectId])],
		recentsExpanded: defaultView.recentsExpanded,
		terminalContexts: defaultView.terminalContexts,
	});
}
