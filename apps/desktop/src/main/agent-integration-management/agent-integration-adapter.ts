import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

export type AgentIntegrationMutationAttempt =
	| { readonly kind: 'attempted' }
	| { readonly kind: 'conflict' | 'failed' | 'manual-action-required' };

export interface AgentIntegrationAdapter {
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;
	disable(): Promise<AgentIntegrationMutationAttempt>;
	enable(): Promise<AgentIntegrationMutationAttempt>;
	inspect(): Promise<DesktopAgentIntegrationSnapshotItem>;
}
