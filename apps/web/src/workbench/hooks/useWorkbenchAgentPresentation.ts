import { useMemoizedFn } from '@orchbee/foundation/react';
import { agentObservationMessages, useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind } from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchAgentProviderIcon } from '../components/workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import { WorkbenchAgentStateGlyph } from '../components/workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
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
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type {
	WorkbenchAgentNames,
	WorkbenchAgentNameSource,
	WorkbenchAgentState,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';
import type { WorkbenchWorkspaceAgentOverviewView } from '../types/workbench-agent-overview';
import { formatWorkbenchAgentLocation } from '../utils/formatWorkbenchAgentLocation';
import {
	formatWorkbenchAgentGroupName,
	formatWorkbenchAgentName,
	listWorkbenchAgentProviderNames,
} from '../utils/workbench-agent-names';
import {
	formatWorkbenchAgentStatus,
	type WorkbenchAgentStatusView,
} from '../utils/formatWorkbenchAgentStatus';
import { subscribeToNothing } from '../utils/subscribeToNothing';
import { useWorkbenchAgentAttention } from './useWorkbenchAgentAttention';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
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
	const terminalTopologyRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
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
			const identityIcon = React.createElement(WorkbenchAgentProviderIcon, {
				agents,
				gradientPrefix: `${agentIconGradientPrefix}-${identityIcons.size}`,
			});
			identityIcons.set(tab.tabId, identityIcon);
			const status = formatWorkbenchAgentStatus(intl, state);
			const hasUnread = presentationIds.some((id) => unread.has(id));
			statusAdornments.set(tab.tabId, {
				accessibleLabel: `${formatWorkbenchAgentGroupName(intl, agents)}: ${status.status}${hasUnread ? ` · ${unreadLabel}` : ''}`,
				mark: React.createElement(WorkbenchAgentStateGlyph, { size: 'small', status }),
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
		if (terminalTopologyRevision !== projectWorkspaceRuntime.getTerminalTopologyRevision()) {
			return [];
		}
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		return projectWorkbenchAgentOverview(snapshot, projectWorkspaceRuntime).map(
			(workspace): WorkbenchWorkspaceAgentOverviewView => {
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
						return {
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
			},
		);
	}, [
		agentNames,
		currentPresentationId,
		foldedWorkspaceIds,
		intl,
		projectWorkspaceRuntime,
		snapshot,
		terminalTopologyRevision,
		unread,
	]);
	return React.useMemo(
		() => ({
			presentationAgentStatuses,
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
			toggleWorkspaceAgents,
			workspaceAgentOverviews,
		}),
		[presentationAgentStatuses, tabPresentation, workspaceAgentOverviews, toggleWorkspaceAgents],
	);
}
