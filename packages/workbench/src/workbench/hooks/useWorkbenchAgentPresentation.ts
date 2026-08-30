import { projectAgentPresentationGroup, type AgentSnapshotSource } from '@orchbee/agent';
import { useOrchbeeIntl } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind, TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchAgentProviderIcon } from '../components/workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import { formatWorkbenchAgentStatus } from '../utils/formatWorkbenchAgentStatus';
import { useAgentSnapshot } from './useAgentSnapshot';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly source: AgentSnapshotSource | undefined;
	readonly tabModel: TabModel;
}

interface WorkbenchAgentPresentationView {
	readonly tabIdentityIcons: ReadonlyMap<string, React.ReactNode>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
}

export function useWorkbenchAgentPresentation({
	notificationObserver,
	panelWorkspaceStore,
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
			if (aggregate === null) continue;
			const identityIcon = React.createElement(WorkbenchAgentProviderIcon, {
				gradientPrefix: `${agentIconGradientPrefix}-${identityIcons.size}`,
				providerIds: aggregate.providerIds,
			});
			identityIcons.set(tab.tabId, identityIcon);
			if (aggregate.status.kind === 'ended') continue;
			const formatted = formatWorkbenchAgentStatus(intl, aggregate);
			let kind: TabStatusAdornmentKind = 'idle';
			if (aggregate.freshness !== 'fresh') kind = 'degraded';
			else if (aggregate.status.kind === 'needs-attention') kind = 'attention';
			else if (aggregate.status.kind === 'failed') kind = 'error';
			else if (aggregate.status.kind === 'succeeded') kind = 'success';
			else if (aggregate.status.kind === 'working') kind = 'active';
			statusAdornments.set(tab.tabId, {
				accessibleLabel: `${formatted.provider}: ${formatted.status}`,
				icon: identityIcon,
				kind,
			});
		}
		return { identityIcons, statusAdornments };
	}, [agentIconGradientPrefix, intl, panelWorkspaceState, snapshot]);
	return React.useMemo(
		() => ({
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
		}),
		[tabPresentation],
	);
}
