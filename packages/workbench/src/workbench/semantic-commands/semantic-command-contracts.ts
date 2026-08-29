import type {
	TerminalPresentationId,
	TerminalPresentationOperationId,
} from '../../terminal-presentation';
import type {
	WorkbenchSharedDocumentRevision,
	WorkbenchSharedPresentationIntent,
} from '../document';

export type WorkbenchLocationRef =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly projectId: string;
			readonly workspaceId: string;
	  };

export interface WorkbenchSemanticReadSnapshot<Item> {
	readonly items: readonly Item[];
	readonly revision: WorkbenchSharedDocumentRevision;
}

export interface WorkbenchProjectListItem {
	readonly name: string;
	readonly projectId: string;
}

export interface WorkbenchDirectoryListItem {
	readonly available: boolean;
	readonly directoryId: string;
	readonly kind: 'folder' | 'worktree';
	readonly name: string;
	readonly path: string;
	readonly primary: boolean;
	readonly source: 'created' | 'discovered' | 'selected';
}

export interface WorkbenchWorkspaceListItem {
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly workspaceId: string;
}

export interface WorkbenchTerminalListItem {
	readonly groupId: string | null;
	readonly intent: WorkbenchSharedPresentationIntent;
	readonly launchCwd: string | null;
	readonly launchDirectoryId: string | null;
	readonly panelId: string;
	readonly pinned: boolean;
	readonly presentationId: TerminalPresentationId;
	readonly role: 'child' | 'primary';
	readonly tabCustomTitle: string | null;
	readonly tabFallbackTitle: string;
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly terminalTitle: string;
}

export type WorkbenchSemanticReadRejection =
	| { readonly kind: 'rejected'; readonly reason: 'project-not-found' }
	| { readonly kind: 'rejected'; readonly reason: 'workspace-not-found' };

export type WorkbenchProjectListResult = WorkbenchSemanticReadSnapshot<WorkbenchProjectListItem>;

export type WorkbenchDirectoryListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchDirectoryListItem>
	| Extract<WorkbenchSemanticReadRejection, { readonly reason: 'project-not-found' }>;

export type WorkbenchWorkspaceListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchWorkspaceListItem>
	| Extract<WorkbenchSemanticReadRejection, { readonly reason: 'project-not-found' }>;

export type WorkbenchTerminalListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchTerminalListItem>
	| WorkbenchSemanticReadRejection;

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

export interface WorkbenchTerminalCreateOperationCreated {
	readonly intent: WorkbenchTerminalCreateIntent;
	readonly kind: 'created';
	readonly panelId: string;
	readonly presentationId: TerminalPresentationId;
	readonly tabId: string;
	readonly terminalContextId: string;
}

export type WorkbenchTerminalCreateRejectionReason =
	| 'explicit-directory-not-found'
	| 'explicit-directory-unavailable'
	| 'project-not-found'
	| 'tab-placement-invalid'
	| 'terminal-creation-sealed'
	| 'terminal-operation-conflict'
	| 'terminal-presentation-conflict'
	| 'workspace-not-found';

export type WorkbenchTerminalCreateOperationResult =
	| WorkbenchTerminalCreateOperationCreated
	| {
			readonly kind: 'rejected';
			readonly reason: WorkbenchTerminalCreateRejectionReason;
	  };
