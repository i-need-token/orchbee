import type { AgentIntegrationId } from '../../agent-integration-management';

export interface AgentIntegrationReminderView {
	readonly actionLabel: string;
	readonly body: string;
	readonly dismissLabel: string;
	readonly integrationId: AgentIntegrationId;
	readonly onAction: () => void;
	readonly onDismiss: () => void;
	readonly title: string;
}
