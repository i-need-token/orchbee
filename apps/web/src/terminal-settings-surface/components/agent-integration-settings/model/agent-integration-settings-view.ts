import { agentIntegrationMessages, type MessageDescriptor } from '@orchbee/i18n';
import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementFailureCode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../../../../agent-integration-management';

export interface AgentIntegrationSettingsRowView {
	readonly agentName: string;
	readonly checked: boolean;
	readonly description: string;
	readonly disabled: boolean;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly onCheckedChange: (checked: boolean) => void;
	readonly pending: boolean;
	readonly settingId: string;
	readonly status: string;
	readonly statusTone: 'danger' | 'neutral' | 'positive' | 'warning';
	readonly toggleLabel: string;
}

export interface AgentIntegrationSettingsView {
	readonly disclosure: string;
	readonly failure: string | null;
	readonly localSection: string;
	readonly onRefresh: () => void;
	readonly refreshDisabled: boolean;
	readonly refreshLabel: string;
	readonly remoteDescription: string;
	readonly remoteTitle: string;
	readonly rows: readonly AgentIntegrationSettingsRowView[];
}

export async function enableAgentIntegration(
	control: AgentIntegrationManagementControl,
	reminderController: AgentIntegrationReminderController | undefined,
	integrationId: DesktopAgentIntegrationId,
): Promise<void> {
	const result = await control.enable(integrationId);
	if (result.kind === 'completed') reminderController?.restore(integrationId);
}

export function resolveAgentIntegrationStatusView(
	integration: DesktopAgentIntegrationSnapshotItem,
	format: (descriptor: MessageDescriptor) => string,
): Pick<AgentIntegrationSettingsRowView, 'status' | 'statusTone'> {
	if (integration.managementMode === 'manual-only') {
		return {
			status: format(agentIntegrationMessages.statusManualOnly),
			statusTone: 'neutral',
		};
	}
	if (integration.issue === 'inspection-failed') {
		return {
			status: format(agentIntegrationMessages.statusInspectionFailed),
			statusTone: 'warning',
		};
	}
	if (integration.availability === 'not-found') {
		return {
			status: format(agentIntegrationMessages.statusNotFound),
			statusTone: 'neutral',
		};
	}
	switch (integration.state) {
		case 'enabled':
			return {
				status: format(agentIntegrationMessages.statusEnabled),
				statusTone: 'positive',
			};
		case 'disabled':
			return {
				status: format(agentIntegrationMessages.statusDisabled),
				statusTone: 'neutral',
			};
		case 'not-installed':
			return {
				status: format(agentIntegrationMessages.statusNotInstalled),
				statusTone: 'neutral',
			};
		case 'conflict':
			return {
				status: format(agentIntegrationMessages.statusConflict),
				statusTone: 'danger',
			};
		case 'unknown':
			return {
				status: format(agentIntegrationMessages.statusUnknown),
				statusTone: 'warning',
			};
	}
}

export function resolveAgentIntegrationFailureMessage(
	code: DesktopAgentIntegrationManagementFailureCode,
): MessageDescriptor {
	switch (code) {
		case 'agent-unavailable':
			return agentIntegrationMessages.failureAgentUnavailable;
		case 'busy':
			return agentIntegrationMessages.failureBusy;
		case 'conflict':
			return agentIntegrationMessages.failureConflict;
		case 'control-unavailable':
			return agentIntegrationMessages.failureControlUnavailable;
		case 'manual-action-required':
			return agentIntegrationMessages.failureManualActionRequired;
		case 'operation-failed':
			return agentIntegrationMessages.failureOperationFailed;
		case 'verification-failed':
			return agentIntegrationMessages.failureVerificationFailed;
	}
}
