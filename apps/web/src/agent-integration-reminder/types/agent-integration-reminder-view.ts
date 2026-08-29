import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';

export interface AgentIntegrationReminderView {
	readonly body: string;
	readonly dismissLabel: string;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly onDismiss: () => void;
	readonly onReview: (returnFocus: HTMLElement) => void;
	readonly reviewLabel: string;
	readonly title: string;
}
