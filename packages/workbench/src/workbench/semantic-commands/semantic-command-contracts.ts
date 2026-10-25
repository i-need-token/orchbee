import type {
	TerminalPresentationId,
	TerminalPresentationOperationId,
} from '../../terminal-presentation';

export type WorkbenchLocationRef =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly workspaceId: string;
	  };

export interface WorkbenchTerminalCreateOperationInput {
	readonly directoryId?: string | undefined;
	readonly fallbackTitle: string;
	readonly location: WorkbenchLocationRef;
	readonly placement: {
		readonly groupId: string | null;
		readonly index?: number | undefined;
	};
}

export type WorkbenchTerminalCreateIntent =
	| { readonly kind: 'preview' }
	| {
			readonly kind: 'provision';
			readonly operationId: TerminalPresentationOperationId;
	  };

interface WorkbenchTerminalCreateOperationCreated {
	readonly intent: WorkbenchTerminalCreateIntent;
	readonly kind: 'created';
	readonly panelId: string;
	readonly presentationId: TerminalPresentationId;
	readonly tabId: string;
	readonly terminalContextId: string;
}

type WorkbenchTerminalCreateRejectionReason =
	| 'explicit-directory-not-found'
	| 'explicit-directory-unavailable'
	| 'project-not-found'
	| 'remote-terminal-context-not-empty'
	| 'tab-placement-invalid'
	| 'terminal-creation-sealed'
	| 'terminal-operation-conflict'
	| 'terminal-presentation-conflict'
	| 'workspace-directory-not-remote'
	| 'workspace-directory-remote'
	| 'workspace-environment-mismatch'
	| 'workspace-not-found';

export type WorkbenchTerminalCreateOperationResult =
	| WorkbenchTerminalCreateOperationCreated
	| {
			readonly kind: 'rejected';
			readonly reason: WorkbenchTerminalCreateRejectionReason;
	  };
