import { agentObservationMessages, useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind, TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchAgentProviderIcon } from '../components/workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import {
	mostUrgentWorkbenchAgentState,
	projectWorkbenchAgentOverview,
	type WorkbenchAgentNavigationInput,
} from '../model/project-workbench-agent-overview';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type {
	WorkbenchAgentState,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';
import type { WorkbenchWorkspaceAgentOverviewView } from '../types/workbench-agent-overview';
import { formatWorkbenchAgentLocation } from '../utils/formatWorkbenchAgentLocation';
import {
	formatWorkbenchAgentGroupName,
	formatWorkbenchAgentName,
} from '../utils/workbench-agent-names';
import { formatWorkbenchAgentStatus } from '../utils/formatWorkbenchAgentStatus';
import { useWorkbenchAgentAttention } from './useWorkbenchAgentAttention';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly root: React.RefObject<HTMLElement | null>;
	readonly source: WorkbenchAgentSource | undefined;
	readonly tabModel: TabModel;
}

interface WorkbenchAgentPresentationView {
	readonly terminalEdited: (presentationId: string) => void;
	readonly tabIdentityIcons: ReadonlyMap<string, React.ReactNode>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
}

const TAB_ADORNMENT_KINDS: Readonly<Record<WorkbenchAgentState, TabStatusAdornmentKind>> = {
	blocked: 'attention',
	done: 'success',
	error: 'error',
	idle: 'idle',
	unknown: 'idle',
	working: 'active',
};

export function useWorkbenchAgentPresentation({
	navigateAgent,
	notificationObserver,
	panelWorkspaceStore,
	projectWorkspaceRuntime,
	root,
	source,
	tabModel,
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
	const {
		presentation: snapshot,
		unread,
		terminalEdited,
	} = useWorkbenchAgentAttention(source, panelWorkspaceStore, tabModel, root, painted);
	const panelWorkspaceState = React.useSyncExternalStore(
		panelWorkspaceStore.subscribe,
		panelWorkspaceStore.getSnapshot,
		panelWorkspaceStore.getSnapshot,
	);
	const terminalTopologyRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
		projectWorkspaceRuntime.getTerminalTopologyRevision,
	);
	const tabPresentation = React.useMemo(() => {
		const identityIcons = new Map<string, React.ReactNode>();
		const statusAdornments = new Map<string, Readonly<TabStatusAdornment>>();
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		for (const tab of panelWorkspaceState.tabs) {
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
				glyph: status.glyph,
				icon: identityIcon,
				kind: TAB_ADORNMENT_KINDS[state],
				unreadLabel: hasUnread ? unreadLabel : undefined,
			});
		}
		return { identityIcons, statusAdornments };
	}, [agentIconGradientPrefix, intl, panelWorkspaceState, snapshot, unread]);
	const workspaceAgentOverviews = React.useMemo(() => {
		if (terminalTopologyRevision !== projectWorkspaceRuntime.getTerminalTopologyRevision()) {
			return [];
		}
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		return projectWorkbenchAgentOverview(snapshot, projectWorkspaceRuntime).map(
			(workspace): WorkbenchWorkspaceAgentOverviewView => {
				const count = workspace.rows.length;
				return {
					agentLabel: formatWorkbenchAgentGroupName(intl, workspace.agents),
					agents: workspace.agents,
					collapseLabel: intl.formatMessage(workbenchMessages.workspaceAgentCollapse, {
						count,
						workspace: workspace.workspaceName,
					}),
					countLabel: intl.formatMessage(workbenchMessages.workspaceAgentCount, { count }),
					expandLabel: intl.formatMessage(workbenchMessages.workspaceAgentExpand, {
						count,
						workspace: workspace.workspaceName,
					}),
					projectId: workspace.projectId,
					rows: workspace.rows.map(({ destination, observation }) => {
						const agentLabel = formatWorkbenchAgentName(intl, observation.agent);
						const locationLabel = formatWorkbenchAgentLocation(intl, destination);
						const status = formatWorkbenchAgentStatus(intl, observation.state);
						return {
							accessibleLabel: intl.formatMessage(workbenchMessages.workspaceAgentNavigationLabel, {
								location: locationLabel,
								provider: agentLabel,
								status: status.status,
							}),
							agentLabel,
							agents: [observation.agent],
							locationLabel,
							presentationId: destination.presentationId,
							status,
							unreadLabel: unread.has(destination.presentationId) ? unreadLabel : undefined,
						};
					}),
					status: formatWorkbenchAgentStatus(intl, workspace.state),
					unreadLabel: workspace.rows.some((row) => unread.has(row.destination.presentationId))
						? unreadLabel
						: undefined,
					visibleCountLabel: count > 99 ? '99+' : String(count),
					workspaceId: workspace.workspaceId,
				};
			},
		);
	}, [intl, projectWorkspaceRuntime, snapshot, terminalTopologyRevision, unread]);
	return React.useMemo(
		() => ({
			terminalEdited,
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
			workspaceAgentOverviews,
		}),
		[tabPresentation, workspaceAgentOverviews, terminalEdited],
	);
}
