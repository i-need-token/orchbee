import { type IntlShape, workbenchMessages } from '@orchbee/i18n';

import type { WorkbenchAgentDestination } from '../model/project-workbench-agent-overview';

/** The Tab, Panel and terminal that hold an Agent, within its Workspace. */
export function formatWorkbenchAgentLocation(
	intl: IntlShape,
	destination: WorkbenchAgentDestination,
): string {
	const terminal =
		destination.terminalPosition.kind === 'primary'
			? intl.formatMessage(workbenchMessages.workspaceAgentLocationPrimaryTerminal)
			: intl.formatMessage(workbenchMessages.workspaceAgentLocationChildTerminal, {
					position: destination.terminalPosition.position,
				});
	return intl.formatMessage(workbenchMessages.workspaceAgentLocation, {
		panel: destination.panelPosition,
		tab: destination.tabTitle,
		terminal,
	});
}
