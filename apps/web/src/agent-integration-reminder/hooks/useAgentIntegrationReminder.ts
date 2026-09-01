import {
	agentIntegrationMessages,
	agentObservationMessages,
	useOrchbeeIntl,
	type MessageDescriptor,
} from '@orchbee/i18n';
import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';
import * as React from 'react';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../../agent-integration-management';
import { isAgentIntegrationReminderEligible } from '../model/isAgentIntegrationReminderEligible';
import type { AgentIntegrationReminderView } from '../types/agent-integration-reminder-view';

const AGENT_NAME_MESSAGES: Readonly<Record<DesktopAgentIntegrationId, MessageDescriptor>> = {
	'claude-code': agentObservationMessages.providerClaudeCode,
	cline: agentObservationMessages.providerCline,
	codex: agentObservationMessages.providerCodex,
	gemini: agentObservationMessages.providerGeminiCli,
	'github-copilot': agentObservationMessages.providerGitHubCopilotCli,
	grok: agentObservationMessages.providerGrok,
	omp: agentObservationMessages.providerOmp,
	opencode: agentObservationMessages.providerOpenCode,
	pi: agentObservationMessages.providerPi,
	'qwen-code': agentObservationMessages.providerQwenCode,
};

export function useAgentIntegrationReminder(
	control: AgentIntegrationManagementControl,
	controller: AgentIntegrationReminderController,
	onOpenSettings: (integrationId: DesktopAgentIntegrationId) => void,
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
	const agentName = intl.formatMessage(AGENT_NAME_MESSAGES[integrationId]);
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
