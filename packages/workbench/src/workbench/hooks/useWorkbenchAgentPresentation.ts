import {
	projectAgentPresentationGroup,
	type AgentLocationPresentation,
	type AgentSnapshotSource,
} from '@orchbee/agent';
import { useOrchbeeIntl } from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind, TabModel } from '@orchbee/tab-strip';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import { formatWorkbenchAgentStatus } from '../utils/formatWorkbenchAgentStatus';
import { projectAgentLocationMap } from '../utils/projectAgentLocationMap';
import { useAgentSnapshot } from './useAgentSnapshot';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly source: AgentSnapshotSource | undefined;
	readonly tabModel: TabModel;
}

interface WorkbenchAgentPresentationView {
	readonly locations: ReadonlyMap<string, AgentLocationPresentation>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
}

export function useWorkbenchAgentPresentation({
	notificationObserver,
	panelWorkspaceStore,
	source,
	tabModel,
}: UseWorkbenchAgentPresentationOptions): WorkbenchAgentPresentationView {
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
	const locations = React.useMemo(() => projectAgentLocationMap(snapshot), [snapshot]);
	const tabStatusAdornments = React.useMemo(() => {
		const result = new Map<string, Readonly<TabStatusAdornment>>();
		for (const tab of panelWorkspaceState.tabs) {
			const presentationIds = collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((presentation) => presentation.presentationId),
			]);
			const aggregate = projectAgentPresentationGroup(snapshot, presentationIds);
			if (aggregate === null || aggregate.status.kind === 'ended') continue;
			const formatted = formatWorkbenchAgentStatus(intl, aggregate);
			let kind: TabStatusAdornmentKind = 'idle';
			if (aggregate.freshness !== 'fresh') kind = 'degraded';
			else if (aggregate.status.kind === 'needs-attention') kind = 'attention';
			else if (aggregate.status.kind === 'failed') kind = 'error';
			else if (aggregate.status.kind === 'succeeded') kind = 'success';
			else if (aggregate.status.kind === 'working') kind = 'active';
			result.set(tab.tabId, {
				accessibleLabel: `${formatted.provider}: ${formatted.status}`,
				icon: React.createElement(ProductIcon, { name: 'agent' }),
				kind,
			});
		}
		return result;
	}, [intl, panelWorkspaceState, snapshot]);
	return React.useMemo(
		() => ({ locations, tabStatusAdornments }),
		[locations, tabStatusAdornments],
	);
}
