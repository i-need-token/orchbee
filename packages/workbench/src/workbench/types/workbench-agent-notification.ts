import type { AgentSemanticEvent } from '@orchbee/agent';

export interface WorkbenchAgentNotificationRequest {
	readonly body: string;
	readonly onActivate: () => void;
	readonly tag: string;
	readonly title: string;
}

export interface WorkbenchAgentEventObservation {
	readonly event: AgentSemanticEvent;
	readonly notification: WorkbenchAgentNotificationRequest;
	readonly visible: boolean;
}

export interface WorkbenchAgentNotificationObserver {
	observeAgentEvent(observation: WorkbenchAgentEventObservation): unknown;
}
