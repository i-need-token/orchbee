import type {
	DesktopSshEnvironmentCatalog,
	DesktopSshEnvironmentId,
} from '@orchbee/desktop-client-protocol';

export type DesktopSshConnectionManagementFailure =
	| 'busy'
	| 'connection'
	| 'control'
	| 'storage'
	| 'target';

type DesktopSshConnectionManagementPendingOperation =
	| { readonly destination: string; readonly kind: 'add' }
	| { readonly environmentId: DesktopSshEnvironmentId; readonly kind: 'disconnect' };

export interface DesktopSshConnectionManagementSnapshot {
	readonly catalog: DesktopSshEnvironmentCatalog;
	readonly failure: DesktopSshConnectionManagementFailure | null;
	readonly pending: DesktopSshConnectionManagementPendingOperation | null;
}

export interface DesktopSshConnectionManagementControl {
	addConnection(destination: string): Promise<boolean>;
	clearFailure(): void;
	connectEnvironment(environmentId: DesktopSshEnvironmentId, environmentName: string): void;
	disconnectEnvironment(environmentId: DesktopSshEnvironmentId): void;
	getManagementSnapshot(): DesktopSshConnectionManagementSnapshot;
	refreshConnections(): void;
	subscribeManagement(listener: () => void): () => void;
}
