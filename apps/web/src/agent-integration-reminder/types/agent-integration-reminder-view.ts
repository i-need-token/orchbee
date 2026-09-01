import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';

export interface AgentIntegrationReminderView {
	readonly actionLabel: string;
	readonly body: string;
	readonly dismissLabel: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly onAction: () => void;
	readonly onDismiss: () => void;
	readonly title: string;
}
