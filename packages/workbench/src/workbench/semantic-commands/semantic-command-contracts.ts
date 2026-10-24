import type {
	TerminalPresentationId,
	TerminalPresentationOperationId,
} from '../../terminal-presentation';
import type {
	WorkbenchSharedDocumentRevision,
	WorkbenchSharedPresentationIntent,
} from '../document';
import type { WorkbenchDirectoryPlacement } from '../../project-workspace';

export type WorkbenchLocationRef =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly workspaceId: string;
	  };

interface WorkbenchSemanticReadSnapshot<Item> {
	readonly items: readonly Item[];
	readonly revision: WorkbenchSharedDocumentRevision;
}

interface WorkbenchProjectListItem {
	readonly name: string;
	readonly projectId: string;
}

interface WorkbenchDirectoryListItem {
	readonly available: boolean;
	readonly directoryId: string;
	readonly kind: 'folder' | 'worktree';
	readonly name: string;
	readonly path: string;
	readonly placement: WorkbenchDirectoryPlacement;
	readonly primary: boolean;
	readonly source: 'created' | 'discovered' | 'selected';
}

interface WorkbenchWorkspaceListItem {
	readonly projectId: string | null;
	readonly resourceScopeId: string;
	readonly name: string;
	readonly primaryDirectoryId: string;
	readonly workspaceId: string;
}

interface WorkbenchTabListItem {
	readonly groupId: string | null;
	readonly panelCount: number;
	readonly pinned: boolean;
	readonly tabCustomTitle: string | null;
	readonly tabFallbackTitle: string;
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly terminalCount: number;
}

export interface WorkbenchPanelListItem {
	readonly panelId: string;
	readonly position: number;
	readonly tabId: string;
	readonly terminalContextId: string;
	readonly terminalCount: number;
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

type WorkbenchSemanticReadRejection =
	| { readonly kind: 'rejected'; readonly reason: 'resource-scope-not-found' }
	| { readonly kind: 'rejected'; readonly reason: 'project-not-found' }
	| { readonly kind: 'rejected'; readonly reason: 'workspace-not-found' }
	| { readonly kind: 'rejected'; readonly reason: 'tab-not-found' };

export type WorkbenchProjectListResult = WorkbenchSemanticReadSnapshot<WorkbenchProjectListItem>;

export type WorkbenchDirectoryListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchDirectoryListItem>
	| Extract<WorkbenchSemanticReadRejection, { readonly reason: 'resource-scope-not-found' }>;

export type WorkbenchWorkspaceListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchWorkspaceListItem>
	| Extract<WorkbenchSemanticReadRejection, { readonly reason: 'project-not-found' }>;

export type WorkbenchTabListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchTabListItem>
	| Exclude<WorkbenchSemanticReadRejection, { readonly reason: 'tab-not-found' }>;

export type WorkbenchPanelListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchPanelListItem>
	| WorkbenchSemanticReadRejection;

export type WorkbenchTerminalListResult =
	| WorkbenchSemanticReadSnapshot<WorkbenchTerminalListItem>
	| Exclude<WorkbenchSemanticReadRejection, { readonly reason: 'tab-not-found' }>;

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
