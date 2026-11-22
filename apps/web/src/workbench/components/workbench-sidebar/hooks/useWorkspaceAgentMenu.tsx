import { agentStartMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { WorkbenchAgentStartProvider } from '@orchbee/workbench-client-protocol';
import { useId, useSyncExternalStore, type ReactNode } from 'react';

import { AgentProviderMark } from '../../../../agent-provider-mark';
import { projectWorkbenchAgentLaunchChoices } from '../../workbench-agent-start-dialog/model/projectWorkbenchAgentLaunchChoices';
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

const LOOKUP_STATUS = {
	'couldnt-check': agentStartMessages.statusCouldntCheck,
	looking: agentStartMessages.statusLooking,
} as const;

export interface WorkspaceAgentChoice {
	readonly label: string;
	readonly providerId: WorkbenchAgentStartProvider;
}

export interface WorkspaceAgentMenu {
	/** The installed Agents a search matches, the Recent first. */
	readonly choices: (query: string) => readonly WorkspaceAgentChoice[];
	/** What an empty list says: no match for the search, or why no Agent is listed. */
	readonly emptyLabel: (query: string) => string;
	readonly label: string;
	readonly mark: (workspaceId: string, providerId: WorkbenchAgentStartProvider) => ReactNode;
	readonly refresh: () => void;
	readonly searchLabel: string;
	readonly start: WorkbenchWorkspaceAgentLaunch['start'];
	/** The row menu's Start Agent submenu, which lists the same Agents. */
	readonly submenu: (workspace: WorkbenchAgentStartWorkspace) => WorkbenchActionMenuEntry;
	/** The list's accessible name, which names the Workspace it starts in. */
	readonly title: (workspace: WorkbenchAgentStartWorkspace) => string;
}

/**
 * Start Agent on a Workspace row: every installed Agent, the Recent first, each starting in a new Tab
 * of that Workspace. The row's + lists them under a search field and its menu lists them in a
 * submenu; while none is found, the list says why.
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
	const choices = (query: string): readonly WorkspaceAgentChoice[] =>
		projectWorkbenchAgentLaunchChoices(catalog, query).map((providerId) => ({
			label: productNames.get(providerId) ?? providerId,
			providerId,
		}));
	const emptyLabel = (query: string): string => {
		if (query.trim() !== '') return intl.formatMessage(agentStartMessages.statusNoMatch);
		const lookup = projectWorkbenchAgentStartRows(catalog, '', false).status;
		return intl.formatMessage(
			lookup === null ? agentStartMessages.statusNoInstalled : LOOKUP_STATUS[lookup],
		);
	};
	const mark = (workspaceId: string, providerId: WorkbenchAgentStartProvider): ReactNode => (
		<AgentProviderMark
			gradientPrefix={`${id}-${workspaceId}-${providerId}`}
			providerId={providerId}
		/>
	);
	const label = intl.formatMessage(agentStartMessages.title);
	return {
		choices,
		emptyLabel,
		label,
		mark,
		refresh: launch.catalog.refresh,
		searchLabel: intl.formatMessage(agentStartMessages.search),
		start: launch.start,
		submenu: (workspace) => {
			const listed = choices('');
			return {
				groups: [
					{
						commands: listed.map(
							(choice): WorkbenchActionMenuCommand => ({
								icon: 'agent',
								id: choice.providerId,
								kind: 'command',
								label: choice.label,
								leadingMark: mark(workspace.id, choice.providerId),
								onSelect: () => launch.start(workspace.id, choice.providerId),
							}),
						),
						id: 'agents',
						label: listed.length === 0 ? emptyLabel('') : undefined,
					},
				],
				icon: 'agent',
				id: 'start-workspace-agent',
				kind: 'submenu',
				label,
			};
		},
		title: (workspace) =>
			intl.formatMessage(agentStartMessages.titleInWorkspace, { name: workspace.name }),
	};
}
