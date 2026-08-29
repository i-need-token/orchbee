import type { WorkbenchSharedDocumentRevision } from '../document';

export type WorkbenchRemoteTerminalCreationFailureReason =
	| 'busy'
	| 'control-unavailable'
	| 'environment-not-connected'
	| 'environment-not-found'
	| 'project-not-found'
	| 'registry-unavailable'
	| 'remote-context-invalid'
	| 'terminal-host-busy'
	| 'terminal-host-compatibility-unsupported'
	| 'terminal-host-unavailable'
	| 'workbench-missing'
	| 'workbench-unavailable'
	| 'workspace-environment-mismatch'
	| 'workspace-not-found';

export type WorkbenchRemoteTerminalCreationResult =
	| {
			readonly kind: 'created' | 'existing';
			readonly panelId: string;
			readonly presentationId: string;
			readonly projectId: string;
			readonly revision: WorkbenchSharedDocumentRevision;
			readonly tabId: string;
			readonly terminalContextId: string;
			readonly workspaceId: string;
	  }
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'failed'; readonly reason: WorkbenchRemoteTerminalCreationFailureReason }
	| {
			readonly kind: 'indeterminate';
			readonly reason: 'control-unavailable' | 'durability-unconfirmed';
	  };

export interface WorkbenchRemoteTerminalCreationCapability {
	create(input: {
		readonly projectId: string;
		readonly signal: AbortSignal;
		readonly workspaceId: string;
	}): Promise<WorkbenchRemoteTerminalCreationResult>;
}
