import type { AgentSnapshotSource } from '@orchbee/agent';
import type { IntlShape } from '@orchbee/i18n';
import type { TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import { projectWorkbenchAgentNotification } from '../utils/projectWorkbenchAgentNotification';

interface UseWorkbenchAgentNotificationsOptions {
	readonly intl: IntlShape;
	readonly observer: WorkbenchAgentNotificationObserver | undefined;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly source: AgentSnapshotSource | undefined;
	readonly tabModel: TabModel;
}

export function useWorkbenchAgentNotifications({
	intl,
	observer,
	panelWorkspaceStore,
	source,
	tabModel,
}: UseWorkbenchAgentNotificationsOptions): void {
	React.useEffect(() => {
		if (observer === undefined || source?.subscribeSemanticEvents === undefined) return;
		return source.subscribeSemanticEvents((event) => {
			const session = source
				.getSnapshot()
				.sessions.find((candidate) => candidate.sessionId === event.sessionId);
			if (session === undefined) return;
			const observation = projectWorkbenchAgentNotification({
				event,
				intl,
				panelWorkspaceStore,
				session,
				tabModel,
			});
			if (observation !== null) observer.observeAgentEvent(observation);
		});
	}, [intl, observer, panelWorkspaceStore, source, tabModel]);
}
