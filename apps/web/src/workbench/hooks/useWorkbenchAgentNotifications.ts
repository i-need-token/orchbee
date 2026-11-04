import { useMemoizedFn } from '@orchbee/foundation/react';
import type { IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import {
	WorkbenchAgentNotificationScheduler,
	type WorkbenchAgentNotificationCandidate,
} from '../model/WorkbenchAgentNotificationScheduler';
import type { WorkbenchAgentNavigationInput } from '../model/project-workbench-agent-overview';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type { WorkbenchAgentSource } from '../types/workbench-agent-observation';
import { projectWorkbenchAgentNotification } from '../utils/projectWorkbenchAgentNotification';

interface UseWorkbenchAgentNotificationsOptions {
	readonly intl: IntlShape;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly observer: WorkbenchAgentNotificationObserver | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly source: WorkbenchAgentSource | undefined;
}

/** Schedules this Client's Agent notifications and returns the paint witness that withholds them. */
export function useWorkbenchAgentNotifications({
	intl,
	navigateAgent,
	observer,
	projectWorkspaceRuntime,
	source,
}: UseWorkbenchAgentNotificationsOptions): WorkbenchAgentNotificationScheduler['painted'] {
	const deliver = useMemoizedFn((candidate: WorkbenchAgentNotificationCandidate): void => {
		const notification = projectWorkbenchAgentNotification({
			candidate,
			intl,
			navigateAgent,
			runtime: projectWorkspaceRuntime,
		});
		if (notification !== null) observer?.observeAgentNotification(notification);
	});
	const scheduler = React.useMemo(
		() => new WorkbenchAgentNotificationScheduler(deliver),
		[deliver],
	);
	React.useEffect(() => {
		if (observer === undefined || source === undefined) return;
		const observe = () => scheduler.observe(source.getSnapshot());
		const unsubscribe = source.subscribe(observe);
		observe();
		return () => {
			unsubscribe();
			scheduler.reset();
		};
	}, [observer, scheduler, source]);
	return scheduler.painted;
}
