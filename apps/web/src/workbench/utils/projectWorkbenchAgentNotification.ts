import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';

import type { WorkbenchAgentNotificationCandidate } from '../model/WorkbenchAgentNotificationScheduler';
import {
	findWorkbenchAgentDestination,
	type WorkbenchAgentNavigationInput,
} from '../model/project-workbench-agent-overview';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAgentNotification } from '../types/workbench-agent-notification';
import { formatWorkbenchAgentLocation } from './formatWorkbenchAgentLocation';
import { formatWorkbenchAgentName } from './workbench-agent-names';

interface ProjectWorkbenchAgentNotificationOptions {
	readonly candidate: WorkbenchAgentNotificationCandidate;
	readonly intl: IntlShape;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

/** A notification about a terminal this Workbench still holds; activating it opens that terminal. */
export function projectWorkbenchAgentNotification({
	candidate,
	intl,
	navigateAgent,
	runtime,
}: ProjectWorkbenchAgentNotificationOptions): WorkbenchAgentNotification | null {
	const destination = findWorkbenchAgentDestination(runtime, candidate.presentationId);
	if (destination === null) return null;
	const { agent, binding, sequence } = candidate.observation;
	const provider = formatWorkbenchAgentName(intl, agent);
	return {
		key: `${destination.presentationId}\0${binding}\0${sequence}`,
		notification: {
			body: intl.formatMessage(agentObservationMessages.notificationBody, {
				location: formatWorkbenchAgentLocation(intl, destination),
				workspace: destination.workspaceName,
			}),
			onActivate: () =>
				navigateAgent({
					presentationId: destination.presentationId,
					workspaceId: destination.workspaceId,
				}),
			// A newer notification about the same terminal replaces the older one.
			tag: `orchbee-agent:${destination.presentationId}`,
			title: intl.formatMessage(
				candidate.kind === 'done'
					? agentObservationMessages.notificationDoneTitle
					: agentObservationMessages.notificationRequestTitle,
				{ provider },
			),
		},
	};
}
