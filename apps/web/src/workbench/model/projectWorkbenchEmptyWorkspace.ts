import { workbenchMessages, workbenchTabMessages, type IntlShape } from '@orchbee/i18n';

import type { ProjectWorkspaceState } from '../../project-workspace';

/** What a shared Workbench shows in place of terminals while its Workspace has no Tabs. */
export interface WorkbenchEmptyWorkspace {
	/** Where a new Tab starts, or that the Workspace has no directory. */
	readonly fact: string;
	/** Names New tab beside its shortcut; null where no Tab can start. */
	readonly newTabLabel: string | null;
}

/**
 * Says what the TUI's empty Workspace says. A shared Workbench's only unavailable directory stands in
 * for a Workspace that has none, where no Tab can start until one is set, which this Client cannot do.
 */
export function projectWorkbenchEmptyWorkspace(
	state: ProjectWorkspaceState,
	intl: IntlShape,
): WorkbenchEmptyWorkspace | null {
	const location = state.activeLocation;
	if (location.kind !== 'workspace') return null;
	const workspace = state.workspaces.find((candidate) => candidate.id === location.workspaceId);
	if (workspace === undefined) return null;
	const directory = state.resourceScopes
		.find((scope) => scope.id === workspace.resourceScopeId)
		?.directories.find((candidate) => candidate.id === workspace.primaryDirectoryId);
	if (directory?.available !== true) {
		return {
			fact: intl.formatMessage(workbenchMessages.workspaceEmptyNoDirectory),
			newTabLabel: null,
		};
	}
	return {
		fact: intl.formatMessage(workbenchMessages.workspaceEmptyDirectory, { path: directory.path }),
		newTabLabel: intl.formatMessage(workbenchTabMessages.newTab),
	};
}
