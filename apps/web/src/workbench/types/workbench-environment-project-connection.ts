type WorkbenchEnvironmentProjectConnectionState =
	| 'blocked'
	| 'connected'
	| 'connecting'
	| 'disconnected'
	| 'interaction-required'
	| 'retrying'
	| 'unavailable';

export interface WorkbenchEnvironmentProjectConnectionStatus {
	readonly projectId: string;
	readonly state: WorkbenchEnvironmentProjectConnectionState;
}

export interface WorkbenchEnvironmentProjectConnectionCapability {
	connect(projectId: string): void;
	disconnect(projectId: string): void;
	getSnapshot(): readonly WorkbenchEnvironmentProjectConnectionStatus[];
	subscribe(listener: () => void): () => void;
}
