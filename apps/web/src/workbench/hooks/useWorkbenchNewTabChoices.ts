import { useOrchbeeIntl, workbenchTabMessages } from '@orchbee/i18n';
import type { TabStripNewTabChoice, TabStripNewTabSection } from '@orchbee/tab-strip';
import type {
	UserLaunchProfile,
	WorkbenchAgentStartProvider,
} from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import { projectWorkbenchAgentStartRows } from '../components/workbench-agent-start-dialog/model/projectWorkbenchAgentStartRows';
import type { WorkbenchAgentCatalogSource } from '../types/workbench-agent-start';
import type { WorkbenchLaunchProfiles } from '../types/workbench-authority';
import { subscribeToNothing } from '../utils/subscribeToNothing';

/** As many Agents as the TUI's New Tab menu lists before More Agents…. */
const AGENT_LIMIT = 4;

interface UseWorkbenchNewTabChoicesOptions {
	/** Absent where no Server publishes the Session's Agent catalog. */
	readonly agentCatalog: WorkbenchAgentCatalogSource | undefined;
	readonly createTab: (profile: UserLaunchProfile) => void;
	readonly inWorkspace: boolean;
	readonly launchProfiles: WorkbenchLaunchProfiles | undefined;
	/** Runs `workbench.agent.start`, which opens Start Agent with New Tab as its default place. */
	readonly openAgentStart: () => void;
	/** Runs `workbench.agent.start-provider` with the new-Tab place. */
	readonly startAgentInNewTab: (providerId: WorkbenchAgentStartProvider) => void;
	/** Start Agent's effective shortcut, absent while it has none. */
	readonly startAgentShortcutLabel: string | undefined;
}

/**
 * The New Tab button's menu, as in the TUI: every saved launch profile, the default first and marked
 * Default, then, while the Session can start an Agent, an Agents section with up to four Agents by
 * product name, the first Recent then Installed rows of Start Agent, and More Agents…. A profile
 * starts one Tab with it and never changes the default; an Agent starts in a new Tab without a
 * question. Without a Workspace there is nowhere to put that Tab, so the button offers no menu.
 */
export function useWorkbenchNewTabChoices({
	agentCatalog,
	createTab,
	inWorkspace,
	launchProfiles,
	openAgentStart,
	startAgentInNewTab,
	startAgentShortcutLabel,
}: UseWorkbenchNewTabChoicesOptions): readonly TabStripNewTabSection[] | undefined {
	const intl = useOrchbeeIntl();
	const readProfiles = () => launchProfiles?.getSnapshot();
	const profileCatalog = React.useSyncExternalStore(
		launchProfiles?.subscribe ?? subscribeToNothing,
		readProfiles,
		readProfiles,
	);
	const readAgents = () => agentCatalog?.getSnapshot() ?? null;
	const agents = React.useSyncExternalStore(
		agentCatalog?.subscribe ?? subscribeToNothing,
		readAgents,
		readAgents,
	);
	return React.useMemo(() => {
		if (profileCatalog === undefined || !inWorkspace) return undefined;
		const { defaultProfileId, profiles } = profileCatalog;
		const profileChoices = [
			...profiles.filter((profile) => profile.id === defaultProfileId),
			...profiles.filter((profile) => profile.id !== defaultProfileId),
		].map(
			(profile): TabStripNewTabChoice => ({
				icon: 'terminal',
				id: profile.id,
				// Settings lists an unnamed profile by what it runs, and so does this menu.
				label:
					profile.name ??
					intl.formatMessage(
						profile.command.kind === 'system-shell'
							? workbenchTabMessages.newTabProfileSystemShell
							: workbenchTabMessages.newTabProfileCustomExecutable,
					),
				...(profile.id === defaultProfileId
					? { note: intl.formatMessage(workbenchTabMessages.newTabProfileDefault) }
					: {}),
				select: () => createTab(profile),
			}),
		);
		const sections: TabStripNewTabSection[] = [{ choices: profileChoices, id: 'profiles' }];
		if (agents?.startable !== true) return sections;
		const productNames = new Map(
			agents.agents.map((entry) => [entry.providerId, entry.productName] as const),
		);
		// A provider still being checked or not found is never listed.
		const agentChoices = projectWorkbenchAgentStartRows(agents, '', false)
			.rows.flatMap((row) =>
				row.kind === 'provider' && (row.group === 'recent' || row.group === 'installed')
					? [row.providerId]
					: [],
			)
			.slice(0, AGENT_LIMIT)
			.map(
				(providerId): TabStripNewTabChoice => ({
					icon: 'agent',
					id: `agent:${providerId}`,
					label: productNames.get(providerId) ?? providerId,
					select: () => startAgentInNewTab(providerId),
				}),
			);
		sections.push({
			choices: [
				...agentChoices,
				{
					icon: 'ellipsis',
					id: 'agent-more',
					label: intl.formatMessage(workbenchTabMessages.newTabMoreAgents),
					...(startAgentShortcutLabel === undefined ? {} : { note: startAgentShortcutLabel }),
					select: openAgentStart,
				},
			],
			id: 'agents',
			label: intl.formatMessage(workbenchTabMessages.newTabAgentsHeading),
		});
		return sections;
	}, [
		agents,
		createTab,
		inWorkspace,
		intl,
		openAgentStart,
		profileCatalog,
		startAgentInNewTab,
		startAgentShortcutLabel,
	]);
}
