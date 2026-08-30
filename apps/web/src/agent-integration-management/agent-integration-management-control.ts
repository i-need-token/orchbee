import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementFailureCode,
	DesktopAgentIntegrationManagementMainEvent,
	DesktopAgentIntegrationManagementSnapshot,
} from '@orchbee/desktop-client-protocol';

export interface AgentIntegrationManagementClientSnapshot extends DesktopAgentIntegrationManagementSnapshot {
	readonly failureCode: DesktopAgentIntegrationManagementFailureCode | null;
	readonly failureIntegrationId: DesktopAgentIntegrationId | null;
	readonly pending: boolean;
	readonly pendingIntegrationId: DesktopAgentIntegrationId | null;
}

export type AgentIntegrationManagementClientInitialization =
	| { readonly kind: 'ready' }
	| { readonly kind: 'unavailable' };

export type AgentIntegrationManagementClientOperationResult =
	| Extract<
			DesktopAgentIntegrationManagementMainEvent,
			{ readonly type: 'desktop-agent-integration-responded' }
	  >['result']
	| { readonly kind: 'unavailable' };

export interface AgentIntegrationManagementControl {
	disable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult>;
	enable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<AgentIntegrationManagementClientOperationResult>;
	getSnapshot(): AgentIntegrationManagementClientSnapshot;
	refresh(): Promise<AgentIntegrationManagementClientOperationResult>;
	subscribe(listener: () => void): () => void;
}
