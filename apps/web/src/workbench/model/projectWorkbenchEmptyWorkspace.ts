import { workbenchMessages, workbenchTabMessages, type IntlShape } from '@orchbee/i18n';

import type { ProjectWorkspaceState } from '../../project-workspace';

/** What a shared Workbench shows in place of terminals while its Workspace has no Tabs. */
export interface WorkbenchEmptyWorkspace {
	/** Where a new Tab starts, or that the Workspace has no directory. */
	readonly fact: string;
	/** Names New tab beside its shortcut; null where no Tab can start. */
	readonly newTabLabel: string | null;
	/** Names Start Agent beside its shortcut; null where no Agent can start. */
	readonly startAgentLabel: string | null;
}

/**
 * Says what the TUI's empty Workspace says: where a Tab starts, then New tab and Start Agent with
 * their keys. A Workspace without a directory starts no Tab until one is set, which this Client
 * cannot do, so it starts no Agent either.
 */
export function projectWorkbenchEmptyWorkspace(
	state: ProjectWorkspaceState,
	intl: IntlShape,
	startAgentOffered: boolean,
): WorkbenchEmptyWorkspace | null {
	const location = state.activeLocation;
	if (location.kind !== 'workspace') return null;
	const workspace = state.workspaces.find((candidate) => candidate.id === location.workspaceId);
	if (workspace === undefined) return null;
	const directory = workspace.directory;
	if (directory === null) {
		return {
			fact: intl.formatMessage(workbenchMessages.workspaceEmptyNoDirectory),
			newTabLabel: null,
			startAgentLabel: null,
		};
	}
	return {
		fact: intl.formatMessage(workbenchMessages.workspaceEmptyDirectory, { path: directory.path }),
		newTabLabel: intl.formatMessage(workbenchTabMessages.newTab),
		startAgentLabel: startAgentOffered
			? intl.formatMessage(workbenchMessages.workspaceEmptyStartAgent)
			: null,
	};
}
