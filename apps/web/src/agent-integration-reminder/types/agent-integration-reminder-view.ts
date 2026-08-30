import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';

export interface AgentIntegrationReminderView {
	readonly body: string;
	readonly dismissLabel: string;
	readonly enableDisabled: boolean;
	readonly enableLabel: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly onDismiss: () => void;
	readonly onEnable: () => void;
	readonly pending: boolean;
	readonly title: string;
}
