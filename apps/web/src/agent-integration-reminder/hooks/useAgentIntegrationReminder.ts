import { agentIntegrationMessages, useOrchbeeIntl } from '@orchbee/i18n';

import * as React from 'react';

import {
	AGENT_INTEGRATION_NAME_MESSAGES,
	type AgentIntegrationId,
	type AgentIntegrationManagementControl,
	type AgentIntegrationReminderController,
} from '../../agent-integration-management';
import { isAgentIntegrationReminderEligible } from '../model/isAgentIntegrationReminderEligible';
import type { AgentIntegrationReminderView } from '../types/agent-integration-reminder-view';

export function useAgentIntegrationReminder(
	control: AgentIntegrationManagementControl,
	controller: AgentIntegrationReminderController,
	onOpenSettings: (integrationId: AgentIntegrationId) => void,
): AgentIntegrationReminderView | null {
	const intl = useOrchbeeIntl();
	const managementSnapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const reminderSnapshot = React.useSyncExternalStore(
		controller.subscribe,
		controller.getSnapshot,
		controller.getSnapshot,
	);
	const activeIntegration =
		reminderSnapshot.activeIntegrationId === null
			? undefined
			: managementSnapshot.integrations.find(
					(integration) => integration.integrationId === reminderSnapshot.activeIntegrationId,
				);
	const candidate = managementSnapshot.launchedIntegrationIds
		.map((integrationId) =>
			managementSnapshot.integrations.find(
				(integration) => integration.integrationId === integrationId,
			),
		)
		.find(
			(integration) =>
				integration !== undefined &&
				isAgentIntegrationReminderEligible(integration) &&
				!reminderSnapshot.dismissedIntegrationIds.includes(integration.integrationId),
		);

	React.useEffect(() => {
		if (
			reminderSnapshot.activeIntegrationId !== null &&
			(activeIntegration === undefined || !isAgentIntegrationReminderEligible(activeIntegration))
		) {
			controller.hide(reminderSnapshot.activeIntegrationId);
			return;
		}
		if (reminderSnapshot.activeIntegrationId === null && candidate !== undefined) {
			controller.offer(candidate.integrationId);
		}
	}, [activeIntegration, candidate, controller, reminderSnapshot.activeIntegrationId]);

	if (activeIntegration === undefined || !isAgentIntegrationReminderEligible(activeIntegration)) {
		return null;
	}
	const integrationId = activeIntegration.integrationId;
	const agentName = intl.formatMessage(AGENT_INTEGRATION_NAME_MESSAGES[integrationId]);
	return {
		actionLabel: intl.formatMessage(agentIntegrationMessages.reminderOpenSettings),
		body: intl.formatMessage(agentIntegrationMessages.reminderBody, { agent: agentName }),
		dismissLabel: intl.formatMessage(agentIntegrationMessages.reminderDismiss, {
			agent: agentName,
		}),
		integrationId,
		onAction: (): void => {
			controller.hide(integrationId);
			onOpenSettings(integrationId);
		},
		onDismiss: (): void => {
			controller.dismiss(integrationId);
		},
		title: intl.formatMessage(agentIntegrationMessages.reminderTitle),
	};
}
