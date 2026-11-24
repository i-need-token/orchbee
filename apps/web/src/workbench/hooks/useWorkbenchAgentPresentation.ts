import { useMemoizedFn } from '@orchbee/foundation/react';
import { agentObservationMessages, useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind } from '@orchbee/tab-strip';
import * as React from 'react';

import { AgentProviderIcon } from '../../agent-provider-icon';
import { WorkbenchAgentStateGlyph } from '../components/workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import {
	collectPanelLeaves,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import {
	mostUrgentWorkbenchAgentState,
	projectWorkbenchAgentOverview,
	type WorkbenchAgentNavigationInput,
} from '../model/project-workbench-agent-overview';
import { resolveWorkbenchAgentTitle } from '../model/resolveWorkbenchAgentTitle';
import { WORKBENCH_AGENT_CONFIRMATION_WAITS } from '../model/workbench-agent-confirmation-news';
import type {
	WorkbenchAgentConfirmation,
	WorkbenchAgentConfirmations,
} from '../types/workbench-agent-confirmations';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type {
	WorkbenchAgentNames,
	WorkbenchAgentNameSource,
	WorkbenchAgentState,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';
import type {
	WorkbenchAgentConfirmationOfferView,
	WorkbenchWorkspaceAgentOverviewView,
} from '../types/workbench-agent-overview';
import { formatWorkbenchAgentLocation } from '../utils/formatWorkbenchAgentLocation';
import {
	formatWorkbenchAgentGroupName,
	formatWorkbenchAgentName,
	listWorkbenchAgentProviderNames,
	workbenchAgentTitleNamesAgent,
} from '../utils/workbench-agent-names';
import {
	formatWorkbenchAgentStatus,
	type WorkbenchAgentStatusView,
} from '../utils/formatWorkbenchAgentStatus';
import { subscribeToNothing } from '../utils/subscribeToNothing';
import { useWorkbenchAgentAttention } from './useWorkbenchAgentAttention';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	/** How the Server answers Agent confirmations; absent where it answers none. */
	readonly confirmations?: WorkbenchAgentConfirmations | undefined;
	/** The names the Server reports for Agents, which an Agent row's title comes from first. */
	readonly nameSource?: WorkbenchAgentNameSource | undefined;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly root: React.RefObject<HTMLElement | null>;
	readonly source: WorkbenchAgentSource | undefined;
	/** The active Workspace's context, whose Tabs carry Agent marks; null while none is open. */
	readonly terminalContext: WorkbenchTerminalContext | null;
}

interface WorkbenchAgentPresentationView {
	/**
	 * The offer the status notice shows: the focused Panel's Agent first, then another Agent the
	 * active Tab shows, as the TUI picks its key's subject.
	 */
	readonly confirmationOffer: WorkbenchAgentConfirmationOfferView | null;
	/** Each reported Agent's status view by Presentation ID; a Panel header draws its displayed terminal's. */
	readonly presentationAgentStatuses: ReadonlyMap<string, WorkbenchAgentStatusView>;
	readonly tabIdentityIcons: ReadonlyMap<string, React.ReactNode>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
	/** Folds or unfolds one Workspace's Agent rows. */
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
}

const NO_PANEL_TABS: readonly [] = [];
const NO_AGENT_NAMES: ReadonlyMap<string, WorkbenchAgentNames> = new Map();
const NO_CONFIRMATIONS: ReadonlyMap<string, WorkbenchAgentConfirmation> = new Map();
const NO_WORKSPACE_AGENT_OVERVIEWS: readonly WorkbenchWorkspaceAgentOverviewView[] = [];

/** The active Tab's focused Panel's terminal: the open Workspace's current Agent, if it is one. */
function focusedPresentationId(
	panelTabs: ReturnType<WorkbenchTerminalContext['panelWorkspaceStore']['getSnapshot']>['tabs'],
	activeTabId: string | null,
): string | null {
	const active = panelTabs.find((tab) => tab.tabId === activeTabId);
	const panel =
		active === undefined
			? undefined
			: collectPanelLeaves(active.root).find((leaf) => leaf.id === active.focusedPanelId);
	return panel === undefined
		? null
		: (panel.activeChildPresentationId ?? panel.primaryPresentation.presentationId);
}

const TAB_ADORNMENT_KINDS: Readonly<Record<WorkbenchAgentState, TabStatusAdornmentKind>> = {
	blocked: 'attention',
	done: 'success',
	error: 'error',
	idle: 'idle',
	opening: 'active',
	unknown: 'idle',
	working: 'active',
};

export function useWorkbenchAgentPresentation({
	confirmations,
	nameSource,
	navigateAgent,
	notificationObserver,
	projectWorkspaceRuntime,
	root,
	source,
	terminalContext,
}: UseWorkbenchAgentPresentationOptions): WorkbenchAgentPresentationView {
	const agentIconGradientPrefix = React.useId().replaceAll(':', '');
	const intl = useOrchbeeIntl();
	const painted = useWorkbenchAgentNotifications({
		intl,
		navigateAgent,
		observer: notificationObserver,
		projectWorkspaceRuntime,
		source,
	});
	const { presentation: snapshot, unread } = useWorkbenchAgentAttention(
		source,
		terminalContext,
		root,
		painted,
	);
	const readAgentNames = () => nameSource?.getSnapshot() ?? NO_AGENT_NAMES;
	const agentNames = React.useSyncExternalStore(
		nameSource?.subscribe ?? subscribeToNothing,
		readAgentNames,
		readAgentNames,
	);
	const readConfirmations = (): ReadonlyMap<string, WorkbenchAgentConfirmation> =>
		confirmations?.getSnapshot() ?? NO_CONFIRMATIONS;
	const confirmationRows = React.useSyncExternalStore(
		confirmations?.subscribe ?? subscribeToNothing,
		readConfirmations,
		readConfirmations,
	);
	const panelWorkspaceStore = terminalContext?.panelWorkspaceStore;
	const readPanelTabs = () => panelWorkspaceStore?.getSnapshot().tabs ?? NO_PANEL_TABS;
	const panelTabs = React.useSyncExternalStore(
		panelWorkspaceStore?.subscribe ?? subscribeToNothing,
		readPanelTabs,
		readPanelTabs,
	);
	const tabModel = terminalContext?.tabModel;
	const readActiveTabId = () => tabModel?.getSnapshot().activeTabId ?? null;
	const activeTabId = React.useSyncExternalStore(
		tabModel?.subscribe ?? subscribeToNothing,
		readActiveTabId,
		readActiveTabId,
	);
	const currentPresentationId = focusedPresentationId(panelTabs, activeTabId);
	const confirmationOffer = React.useMemo((): WorkbenchAgentConfirmationOfferView | null => {
		const active = panelTabs.find((tab) => tab.tabId === activeTabId);
		if (active === undefined) return null;
		const shown = collectPanelLeaves(active.root).map(
			(panel) => getDisplayedTerminal(panel).presentationId,
		);
		const focused = shown.filter((presentationId) => presentationId === currentPresentationId);
		for (const presentationId of [...focused, ...shown]) {
			const confirmation = confirmationRows.get(presentationId);
			if (confirmation?.offer === true) {
				return {
					agentLabel: formatWorkbenchAgentName(intl, confirmation.agent),
					presentationId,
				};
			}
		}
		return null;
	}, [activeTabId, confirmationRows, currentPresentationId, intl, panelTabs]);
	const terminalLayoutRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalLayoutRevision,
		projectWorkspaceRuntime.getTerminalLayoutRevision,
	);
	// As in the TUI, every Workspace shows its Agents until this Client folds them, and a fold lasts
	// while its Workspace does, through a hidden sidebar and Agents that come and go.
	const [foldedWorkspaceIds, setFoldedWorkspaceIds] = React.useState<ReadonlySet<string>>(
		() => new Set(),
	);
	const toggleWorkspaceAgents = useMemoizedFn((workspaceId: string): void => {
		const present = new Set(projectWorkspaceRuntime.getSnapshot().workspaces.map(({ id }) => id));
		setFoldedWorkspaceIds((current) => {
			const next = new Set([...current].filter((id) => present.has(id)));
			if (!next.delete(workspaceId)) next.add(workspaceId);
			return next;
		});
	});
	const tabPresentation = React.useMemo(() => {
		const identityIcons = new Map<string, React.ReactNode>();
		const statusAdornments = new Map<string, Readonly<TabStatusAdornment>>();
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		for (const tab of panelTabs) {
			const presentationIds = collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((presentation) => presentation.presentationId),
			]);
			const observations = presentationIds.flatMap((id) => {
				const observation = snapshot.presentations.get(id);
				return observation === undefined ? [] : [observation];
			});
			const state = mostUrgentWorkbenchAgentState(
				observations.map((observation) => observation.state),
			);
			if (state === null) continue;
			const agents = [...new Set(observations.map((observation) => observation.agent))];
			const identityIcon = React.createElement(AgentProviderIcon, {
				agents,
				gradientPrefix: `${agentIconGradientPrefix}-${identityIcons.size}`,
			});
			identityIcons.set(tab.tabId, identityIcon);
			const status = formatWorkbenchAgentStatus(intl, state);
			const hasUnread = presentationIds.some((id) => unread.has(id));
			statusAdornments.set(tab.tabId, {
				accessibleLabel: `${formatWorkbenchAgentGroupName(intl, agents)}: ${status.status}${hasUnread ? ` · ${unreadLabel}` : ''}`,
				mark: React.createElement(WorkbenchAgentStateGlyph, { status }),
				icon: identityIcon,
				kind: TAB_ADORNMENT_KINDS[state],
				unreadLabel: hasUnread ? unreadLabel : undefined,
			});
		}
		return { identityIcons, statusAdornments };
	}, [agentIconGradientPrefix, intl, panelTabs, snapshot, unread]);
	const presentationAgentStatuses = React.useMemo(() => {
		const statuses = new Map<string, WorkbenchAgentStatusView>();
		for (const [presentationId, observation] of snapshot.presentations) {
			statuses.set(presentationId, formatWorkbenchAgentStatus(intl, observation.state));
		}
		return statuses;
	}, [intl, snapshot]);
	const workspaceAgentOverviews = React.useMemo(() => {
		if (terminalLayoutRevision !== projectWorkspaceRuntime.getTerminalLayoutRevision()) {
			return NO_WORKSPACE_AGENT_OVERVIEWS;
		}
		const overview = projectWorkbenchAgentOverview(snapshot, projectWorkspaceRuntime);
		// No Agents keeps one empty list, so the sidebar sees no change from a Tab switch.
		if (overview.length === 0) return NO_WORKSPACE_AGENT_OVERVIEWS;
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		return overview.map((workspace): WorkbenchWorkspaceAgentOverviewView => {
			const count = workspace.rows.length;
			const states = workspace.rowsByUrgency.map(({ observation }) => observation.state);
			// A row's name as its expanded row shows it: its title, then its provider.
			const rowNames = ({ destination, observation }: (typeof workspace.rows)[number]) => {
				const agentLabel = formatWorkbenchAgentName(intl, observation.agent);
				const names = agentNames.get(destination.presentationId);
				const titleLabel =
					resolveWorkbenchAgentTitle(
						destination.terminalNames,
						// What the Server read for another incarnation names nothing in this one.
						names?.binding === observation.binding ? names : undefined,
						listWorkbenchAgentProviderNames(intl, observation.agent),
						workbenchAgentTitleNamesAgent(observation.agent),
					) ?? '';
				return { agentLabel, titleLabel };
			};
			return {
				agentLabel: formatWorkbenchAgentGroupName(intl, workspace.agents),
				collapseLabel: intl.formatMessage(workbenchMessages.workspaceAgentCollapse, {
					count,
					workspace: workspace.workspaceName,
				}),
				countLabel: intl.formatMessage(workbenchMessages.workspaceAgentCount, { count }),
				expandLabel: intl.formatMessage(workbenchMessages.workspaceAgentExpand, {
					count,
					workspace: workspace.workspaceName,
				}),
				expanded: !foldedWorkspaceIds.has(workspace.workspaceId),
				foldedStatus: {
					glyphs: workspace.rowsByUrgency.map(({ destination, observation }) => {
						const { agentLabel, titleLabel } = rowNames({ destination, observation });
						const status = formatWorkbenchAgentStatus(intl, observation.state);
						return {
							identity: titleLabel === '' ? agentLabel : `${titleLabel} · ${agentLabel}`,
							presentationId: destination.presentationId,
							shape: status.shape,
							status: status.status,
							tone: status.tone,
						};
					}),
					label: intl.formatList(
						[...new Set(states)].map((state) => formatWorkbenchAgentStatus(intl, state).status),
						{ style: 'narrow', type: 'conjunction' },
					),
					legend: [...new Set(states)].map((state) => formatWorkbenchAgentStatus(intl, state)),
				},
				projectId: workspace.projectId,
				rows: workspace.rows.map(({ destination, observation }) => {
					const { agentLabel, titleLabel } = rowNames({ destination, observation });
					const locationLabel = formatWorkbenchAgentLocation(intl, destination);
					const status = formatWorkbenchAgentStatus(intl, observation.state);
					const confirmation = confirmationRows.get(destination.presentationId);
					return {
						confirmation:
							confirmation === undefined
								? undefined
								: {
										agentTypeLabel: intl.formatMessage(
											workbenchMessages.agentConfirmationAgentType,
											{ agent: formatWorkbenchAgentName(intl, confirmation.agent) },
										),
										choice: confirmation.choice,
									},
						statusReason:
							confirmation === undefined || confirmation.wait === null
								? undefined
								: intl.formatMessage(workbenchMessages.agentConfirmationWaitStatus, {
										reason: intl.formatMessage(
											WORKBENCH_AGENT_CONFIRMATION_WAITS[confirmation.wait],
										),
										status: status.status,
									}),
						accessibleLabel: intl.formatMessage(workbenchMessages.workspaceAgentNavigationLabel, {
							location: locationLabel,
							provider: agentLabel,
							status: status.status,
						}),
						agentLabel,
						current: destination.presentationId === currentPresentationId,
						panelId: destination.panelId,
						presentationId: destination.presentationId,
						primary: destination.terminalPosition.kind === 'primary',
						providerId: observation.agent,
						status,
						tabId: destination.tabId,
						terminalContextId: destination.terminalContextId,
						titleLabel,
						unreadLabel: unread.has(destination.presentationId) ? unreadLabel : undefined,
					};
				}),
				status: formatWorkbenchAgentStatus(intl, workspace.state),
				unreadLabel: workspace.rows.some((row) => unread.has(row.destination.presentationId))
					? unreadLabel
					: undefined,
				workspaceId: workspace.workspaceId,
			};
		});
	}, [
		agentNames,
		confirmationRows,
		currentPresentationId,
		foldedWorkspaceIds,
		intl,
		projectWorkspaceRuntime,
		snapshot,
		terminalLayoutRevision,
		unread,
	]);
	return React.useMemo(
		() => ({
			confirmationOffer,
			presentationAgentStatuses,
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
			toggleWorkspaceAgents,
			workspaceAgentOverviews,
		}),
		[
			confirmationOffer,
			presentationAgentStatuses,
			tabPresentation,
			workspaceAgentOverviews,
			toggleWorkspaceAgents,
		],
	);
}
