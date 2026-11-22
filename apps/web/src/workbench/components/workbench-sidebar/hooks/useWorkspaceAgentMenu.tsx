import { agentStartMessages, useOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import { useId, useSyncExternalStore } from 'react';

import { AgentProviderIcon } from '../../../../agent-provider-icon';
import { projectWorkbenchAgentStartRows } from '../../workbench-agent-start-dialog/model/projectWorkbenchAgentStartRows';
import type {
	WorkbenchAgentStartWorkspace,
	WorkbenchWorkspaceAgentLaunch,
} from '../../../types/workbench-agent-start';
import type {
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../../types/workbench-action-menu';
import { subscribeToNothing } from '../../../utils/subscribeToNothing';

export interface WorkspaceAgentMenu {
	readonly label: string;
	readonly entries: (
		workspace: WorkbenchAgentStartWorkspace,
		returnTo: () => HTMLElement | null,
	) => readonly WorkbenchActionMenuEntry[];
	readonly open: WorkbenchWorkspaceAgentLaunch['open'];
	readonly refresh: () => void;
}

const QUICK_CHOICES = 4;
const CATALOG_STATUS = {
	looking: agentStartMessages.statusLooking,
	'couldnt-check': agentStartMessages.statusCouldntCheck,
	empty: agentStartMessages.statusNoInstalled,
} as const;

/** The Workspace shortcut to Start Agent uses the same installed-only projection as its picker. */
export function useWorkspaceAgentMenu(
	launch: WorkbenchWorkspaceAgentLaunch | undefined,
): WorkspaceAgentMenu | undefined {
	const intl = useOrchbeeIntl();
	const id = useId();
	const read = () => launch?.catalog.getSnapshot() ?? null;
	const catalog = useSyncExternalStore(launch?.catalog.subscribe ?? subscribeToNothing, read, read);
	if (launch === undefined || catalog?.startable !== true) return undefined;
	const projection = projectWorkbenchAgentStartRows(catalog, '', false, true);
	const agents = new Map(catalog.agents.map((agent) => [agent.providerId, agent.productName]));
	const rows = projection.rows.flatMap((row) => (row.kind === 'provider' ? [row] : []));
	const shortlist = rows.slice(0, QUICK_CHOICES);
	const label = intl.formatMessage(agentStartMessages.title);
	return {
		label,
		open: launch.open,
		refresh: launch.catalog.refresh,
		entries: (workspace, returnTo) => {
			const entries: WorkbenchActionMenuEntry[] = [];
			for (const [group, title] of [
				['recent', agentStartMessages.groupRecent],
				['installed', agentStartMessages.groupInstalled],
			] as const) {
				const commands: WorkbenchActionMenuCommand[] = shortlist
					.filter((row) => row.group === group)
					.map(({ providerId }) => ({
						icon: 'agent',
						id: providerId,
						kind: 'command',
						label: agents.get(providerId) ?? providerId,
						leadingMark: (
							<AgentProviderIcon
								agents={[providerId]}
								gradientPrefix={`${id}-${workspace.id}-${providerId}`}
							/>
						),
						onSelect: () => launch.start(workspace.id, providerId),
					}));
				if (commands.length > 0) {
					entries.push({ commands, id: group, kind: 'group', label: intl.formatMessage(title) });
				}
			}
			if (rows.length === 0) {
				const status = CATALOG_STATUS[projection.status ?? 'empty'];
				entries.push({
					commands: [],
					id: 'status',
					kind: 'group',
					label: intl.formatMessage(status),
				});
			}
			if (rows.length === 0 || rows.length > QUICK_CHOICES) {
				entries.push(
					{ id: 'more-separator', kind: 'separator' },
					{
						icon: 'ellipsis',
						id: 'more',
						kind: 'command',
						label: intl.formatMessage(workbenchTabMessages.newTabMoreAgents),
						onSelect: () => launch.open(workspace, returnTo()),
					},
				);
			}
			return entries;
		},
	};
}
