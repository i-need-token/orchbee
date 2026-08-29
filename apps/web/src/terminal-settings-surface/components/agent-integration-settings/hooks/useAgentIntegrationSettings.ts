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
} from '../../../../agent-integration-management';
import {
	enableAgentIntegration,
	resolveAgentIntegrationFailureMessage,
	resolveAgentIntegrationStatusView,
	type AgentIntegrationSettingsRowView,
	type AgentIntegrationSettingsView,
} from '../model/agent-integration-settings-view';

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

export function useAgentIntegrationSettings(
	control: AgentIntegrationManagementControl,
	reminderController?: AgentIntegrationReminderController | undefined,
): AgentIntegrationSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const rows = snapshot.integrations.map((integration): AgentIntegrationSettingsRowView => {
		const agentName = intl.formatMessage(AGENT_NAME_MESSAGES[integration.integrationId]);
		const pending =
			snapshot.pending &&
			(snapshot.pendingIntegrationId === null ||
				snapshot.pendingIntegrationId === integration.integrationId);
		const enabled = integration.state === 'enabled';
		const disabled =
			snapshot.pending ||
			integration.managementMode === 'manual-only' ||
			integration.state === 'conflict' ||
			integration.state === 'unknown' ||
			(!enabled && integration.availability !== 'available');
		const statusView = pending
			? {
					status: intl.formatMessage(agentIntegrationMessages.actionWorking),
					statusTone: 'neutral' as const,
				}
			: resolveAgentIntegrationStatusView(integration, (descriptor) =>
					intl.formatMessage(descriptor),
				);
		return {
			agentName,
			checked: enabled,
			description: intl.formatMessage(agentIntegrationMessages.providerDescription, {
				agent: agentName,
			}),
			disabled,
			integrationId: integration.integrationId,
			onCheckedChange: (checked): void => {
				if (disabled || checked === enabled) return;
				if (!checked) {
					reminderController?.dismiss(integration.integrationId);
					void control.disable(integration.integrationId);
					return;
				}
				void enableAgentIntegration(control, reminderController, integration.integrationId);
			},
			pending,
			settingId: `agent-integrations.${integration.integrationId}`,
			...statusView,
			toggleLabel: intl.formatMessage(agentIntegrationMessages.toggleLabel, {
				agent: agentName,
			}),
		};
	});
	return {
		disclosure: intl.formatMessage(agentIntegrationMessages.disclosure),
		failure:
			snapshot.failureCode === null
				? null
				: intl.formatMessage(resolveAgentIntegrationFailureMessage(snapshot.failureCode)),
		localSection: intl.formatMessage(agentIntegrationMessages.localSection),
		onRefresh: (): void => {
			if (!snapshot.pending) void control.refresh();
		},
		refreshDisabled: snapshot.pending,
		refreshLabel: intl.formatMessage(agentIntegrationMessages.actionRefresh),
		remoteDescription: intl.formatMessage(agentIntegrationMessages.remoteDescription),
		remoteTitle: intl.formatMessage(agentIntegrationMessages.remoteTitle),
		rows,
	};
}
