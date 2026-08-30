import { projectAgentPresentationGroup, type AgentSnapshotSource } from '@orchbee/agent';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind, TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchAgentProviderIcon } from '../components/workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import { projectWorkbenchAgentOverview } from '../model/project-workbench-agent-overview';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type { WorkbenchWorkspaceAgentOverviewView } from '../types/workbench-agent-overview';
import { formatWorkbenchAgentStatus } from '../utils/formatWorkbenchAgentStatus';
import { useAgentSnapshot } from './useAgentSnapshot';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly source: AgentSnapshotSource | undefined;
	readonly tabModel: TabModel;
}

interface WorkbenchAgentPresentationView {
	readonly tabIdentityIcons: ReadonlyMap<string, React.ReactNode>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
}

export function useWorkbenchAgentPresentation({
	notificationObserver,
	panelWorkspaceStore,
	projectWorkspaceRuntime,
	source,
	tabModel,
}: UseWorkbenchAgentPresentationOptions): WorkbenchAgentPresentationView {
	const agentIconGradientPrefix = React.useId().replaceAll(':', '');
	const intl = useOrchbeeIntl();
	const snapshot = useAgentSnapshot(source);
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
	useWorkbenchAgentNotifications({
		intl,
		observer: notificationObserver,
		panelWorkspaceStore,
		source,
		tabModel,
	});
	const tabPresentation = React.useMemo(() => {
		const identityIcons = new Map<string, React.ReactNode>();
		const statusAdornments = new Map<string, Readonly<TabStatusAdornment>>();
		for (const tab of panelWorkspaceState.tabs) {
			const presentationIds = collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((presentation) => presentation.presentationId),
			]);
			const aggregate = projectAgentPresentationGroup(snapshot, presentationIds);
			if (aggregate === null || aggregate.status.kind === 'ended') continue;
			const identityIcon = React.createElement(WorkbenchAgentProviderIcon, {
				gradientPrefix: `${agentIconGradientPrefix}-${identityIcons.size}`,
				providerIds: aggregate.providerIds,
			});
			identityIcons.set(tab.tabId, identityIcon);
			const formatted = formatWorkbenchAgentStatus(intl, aggregate);
			let kind: TabStatusAdornmentKind = 'idle';
			if (aggregate.freshness !== 'fresh') kind = 'degraded';
			else if (aggregate.status.kind === 'needs-attention') kind = 'attention';
			else if (
				aggregate.status.kind === 'failed' ||
				aggregate.status.kind === 'canceled' ||
				aggregate.status.kind === 'interrupted'
			) {
				kind = 'error';
			} else if (aggregate.status.kind === 'succeeded') kind = 'success';
			else if (aggregate.status.kind === 'working') kind = 'active';
			statusAdornments.set(tab.tabId, {
				accessibleLabel: `${formatted.provider}: ${formatted.status}`,
				icon: identityIcon,
				kind,
			});
		}
		return { identityIcons, statusAdornments };
	}, [agentIconGradientPrefix, intl, panelWorkspaceState, snapshot]);
	const workspaceAgentOverviews = React.useMemo(() => {
		if (terminalTopologyRevision !== projectWorkspaceRuntime.getTerminalTopologyRevision()) {
			return [];
		}
		const runtimeSnapshot = projectWorkspaceRuntime.getSnapshot();
		const workspaceNames = new Map(
			runtimeSnapshot.projects.flatMap((project) =>
				project.workspaces.map(
					(workspace) => [`${project.id}\u0000${workspace.id}`, workspace.name] as const,
				),
			),
		);
		return projectWorkbenchAgentOverview(snapshot, projectWorkspaceRuntime).workspaces.map(
			(workspace): WorkbenchWorkspaceAgentOverviewView => {
				const workspaceName =
					workspaceNames.get(`${workspace.projectId}\u0000${workspace.workspaceId}`) ?? '';
				const countLabel = intl.formatMessage(workbenchMessages.workspaceAgentCount, {
					count: workspace.sessionCount,
				});
				return {
					collapseLabel: intl.formatMessage(workbenchMessages.workspaceAgentCollapse, {
						count: workspace.sessionCount,
						workspace: workspaceName,
					}),
					countLabel,
					expandLabel: intl.formatMessage(workbenchMessages.workspaceAgentExpand, {
						count: workspace.sessionCount,
						workspace: workspaceName,
					}),
					projectId: workspace.projectId,
					providerIds: workspace.providerIds,
					rows: workspace.sessions.map((session) => {
						const status = formatWorkbenchAgentStatus(intl, {
							freshness: session.freshness,
							providerIds: [session.providerId],
							status: session.status,
						});
						const terminal = intl.formatMessage(
							session.destination.terminalPosition.kind === 'primary'
								? workbenchMessages.workspaceAgentLocationPrimaryTerminal
								: workbenchMessages.workspaceAgentLocationChildTerminal,
							session.destination.terminalPosition.kind === 'primary'
								? undefined
								: { position: session.destination.terminalPosition.position },
						);
						const locationLabel = intl.formatMessage(workbenchMessages.workspaceAgentLocation, {
							panel: session.destination.panelPosition,
							tab: session.destination.tabTitle,
							terminal,
						});
						const navigationLabel = intl.formatMessage(
							workbenchMessages.workspaceAgentNavigationLabel,
							{
								location: locationLabel,
								provider: status.provider,
								status: status.status,
							},
						);
						return {
							accessibleLabel: session.destination.navigable
								? navigationLabel
								: intl.formatMessage(workbenchMessages.workspaceAgentNotNavigable, {
										label: navigationLabel,
									}),
							locationLabel,
							navigable: session.destination.navigable,
							providerIds: [session.providerId],
							providerLabel: status.provider,
							sessionId: session.sessionId,
							status,
						};
					}),
					sessionCount: workspace.sessionCount,
					status: formatWorkbenchAgentStatus(intl, workspace),
					visibleCountLabel: workspace.sessionCount > 99 ? '99+' : String(workspace.sessionCount),
					workspaceId: workspace.workspaceId,
				};
			},
		);
	}, [intl, projectWorkspaceRuntime, snapshot, terminalTopologyRevision]);
	return React.useMemo(
		() => ({
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
			workspaceAgentOverviews,
		}),
		[tabPresentation, workspaceAgentOverviews],
	);
}
