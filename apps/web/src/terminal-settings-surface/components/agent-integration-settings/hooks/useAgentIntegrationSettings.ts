import { agentIntegrationMessages, useOrchbeeIntl } from '@orchbee/i18n';

import * as React from 'react';

import {
	AGENT_INTEGRATION_NAME_MESSAGES,
	type AgentIntegrationManagementControl,
} from '../../../../agent-integration-management';
import {
	resolveAgentIntegrationFailureMessage,
	resolveAgentIntegrationStatusView,
	type AgentIntegrationSettingsRowView,
	type AgentIntegrationSettingsView,
} from '../model/agent-integration-settings-view';

export function useAgentIntegrationSettings(
	control: AgentIntegrationManagementControl,
): AgentIntegrationSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	const rows = snapshot.integrations.map((integration): AgentIntegrationSettingsRowView => {
		const agentName = intl.formatMessage(
			AGENT_INTEGRATION_NAME_MESSAGES[integration.integrationId],
		);
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
				void (checked
					? control.enable(integration.integrationId)
					: control.disable(integration.integrationId));
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
