import { agentStartMessages, useOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import { useId, useSyncExternalStore } from 'react';

import { AgentProviderMark } from '../../../../agent-provider-mark';
import { projectWorkbenchAgentShortlist } from '../../workbench-agent-start-dialog/model/projectWorkbenchAgentShortlist';
import { projectWorkbenchAgentStartRows } from '../../workbench-agent-start-dialog/model/projectWorkbenchAgentStartRows';
import type {
	WorkbenchAgentStartWorkspace,
	WorkbenchWorkspaceAgentLaunch,
} from '../../../types/workbench-agent-start';
import type { WorkbenchActionMenuEntry } from '../../../types/workbench-action-menu';
import { subscribeToNothing } from '../../../utils/subscribeToNothing';

const LOOKUP_STATUS = {
	'couldnt-check': agentStartMessages.statusCouldntCheck,
	looking: agentStartMessages.statusLooking,
} as const;

export interface WorkspaceAgentMenu {
	readonly label: string;
	readonly entries: (
		workspace: WorkbenchAgentStartWorkspace,
		returnTo: () => HTMLElement | null,
	) => readonly WorkbenchActionMenuEntry[];
	readonly open: WorkbenchWorkspaceAgentLaunch['open'];
	readonly refresh: () => void;
}

/**
 * The Workspace row's + menu: the Agents the New Tab menu offers, each starting in a new Tab of
 * that Workspace, then More Agents…, which opens Start Agent for that Workspace. While no Agent is
 * found yet, one line above More Agents… says why.
 */
export function useWorkspaceAgentMenu(
	launch: WorkbenchWorkspaceAgentLaunch | undefined,
): WorkspaceAgentMenu | undefined {
	const intl = useOrchbeeIntl();
	const id = useId();
	const read = () => launch?.catalog.getSnapshot() ?? null;
	const catalog = useSyncExternalStore(launch?.catalog.subscribe ?? subscribeToNothing, read, read);
	if (launch === undefined || catalog?.startable !== true) return undefined;
	const productNames = new Map(
		catalog.agents.map((agent) => [agent.providerId, agent.productName]),
	);
	const shortlist = projectWorkbenchAgentShortlist(catalog);
	const lookup =
		shortlist.length === 0 ? projectWorkbenchAgentStartRows(catalog, '', false).status : null;
	return {
		label: intl.formatMessage(agentStartMessages.title),
		open: launch.open,
		refresh: launch.catalog.refresh,
		entries: (workspace, returnTo) => [
			...shortlist.map(
				(providerId): WorkbenchActionMenuEntry => ({
					icon: 'agent',
					id: providerId,
					kind: 'command',
					label: productNames.get(providerId) ?? providerId,
					leadingMark: (
						<AgentProviderMark
							gradientPrefix={`${id}-${workspace.id}-${providerId}`}
							providerId={providerId}
						/>
					),
					onSelect: () => launch.start(workspace.id, providerId),
				}),
			),
			...(shortlist.length === 0
				? [
						{
							commands: [],
							id: 'status',
							kind: 'group',
							label: intl.formatMessage(
								lookup === null ? agentStartMessages.statusNoInstalled : LOOKUP_STATUS[lookup],
							),
						} as const,
					]
				: [{ id: 'more-separator', kind: 'separator' } as const]),
			{
				icon: 'ellipsis',
				id: 'more',
				kind: 'command',
				label: intl.formatMessage(workbenchTabMessages.newTabMoreAgents),
				onSelect: () => launch.open(workspace, returnTo()),
			},
		],
	};
}
