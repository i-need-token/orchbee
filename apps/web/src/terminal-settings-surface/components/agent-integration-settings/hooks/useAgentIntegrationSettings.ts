import { agentIntegrationMessages, type MessageDescriptor, useOrchbeeIntl } from '@orchbee/i18n';

import * as React from 'react';

import {
	AGENT_INTEGRATION_NAME_MESSAGES,
	explainStartsAgent,
	type AgentIntegrationId,
	type AgentIntegrationManagementControl,
} from '../../../../agent-integration-management';
import {
	resolveAgentIntegrationConsentView,
	resolveAgentIntegrationDiagnosisView,
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
	// Which Agent's diagnosis this mount shows; the diagnosis itself belongs to the control.
	const [shownDiagnosis, setShownDiagnosis] = React.useState<AgentIntegrationId | null>(null);
	// The Agent whose check would start its program and waits for a second Explain; nothing runs yet.
	const [asking, setAsking] = React.useState<AgentIntegrationId | null>(null);
	const format = (
		descriptor: MessageDescriptor,
		values?: Readonly<Record<string, string>>,
	): string => intl.formatMessage(descriptor, values);
	const formatList = (items: readonly string[]): string =>
		intl.formatList(items, { type: 'conjunction' });
	const { diagnosis } = snapshot;
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
		const shown =
			shownDiagnosis === integration.integrationId &&
			diagnosis?.integrationId === integration.integrationId
				? diagnosis
				: null;
		let diagnosisView: AgentIntegrationSettingsRowView['diagnosis'] = null;
		if (asking === integration.integrationId) {
			diagnosisView = resolveAgentIntegrationConsentView(agentName, format);
		} else if (shown !== null) {
			diagnosisView = resolveAgentIntegrationDiagnosisView(shown, format, formatList);
		}
		return {
			agentName,
			checked: enabled,
			description: intl.formatMessage(agentIntegrationMessages.providerDescription, {
				agent: agentName,
			}),
			diagnosis: diagnosisView,
			diagnosisTitle: intl.formatMessage(agentIntegrationMessages.diagnosisTitle, {
				agent: agentName,
			}),
			disabled,
			explainDisabled: snapshot.pending,
			explainLabel: intl.formatMessage(agentIntegrationMessages.explainLabel, {
				agent: agentName,
			}),
			integrationId: integration.integrationId,
			onCheckedChange: (checked): void => {
				if (disabled || checked === enabled) return;
				void (checked
					? control.enable(integration.integrationId)
					: control.disable(integration.integrationId));
			},
			onExplain: (): void => {
				if (snapshot.pending) return;
				// A check that starts the Agent's program first says so; Explain on that question,
				// or on the open diagnosis it produced, is the consent that runs it.
				if (
					shown === null &&
					asking !== integration.integrationId &&
					explainStartsAgent(integration)
				) {
					setAsking(integration.integrationId);
					setShownDiagnosis(null);
					return;
				}
				setAsking(null);
				setShownDiagnosis(integration.integrationId);
				void control.explain(integration.integrationId);
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
		closeDiagnosisLabel: intl.formatMessage(agentIntegrationMessages.diagnosisClose),
		disclosure: intl.formatMessage(agentIntegrationMessages.disclosure),
		explainText: intl.formatMessage(agentIntegrationMessages.actionExplain),
		failure:
			snapshot.failureCode === null
				? null
				: intl.formatMessage(resolveAgentIntegrationFailureMessage(snapshot.failureCode)),
		localSection: intl.formatMessage(agentIntegrationMessages.localSection),
		onCloseDiagnosis: (): void => {
			setAsking(null);
			setShownDiagnosis(null);
		},
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
