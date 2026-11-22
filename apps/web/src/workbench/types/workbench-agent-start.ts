import type {
	WorkbenchAgentCatalog,
	WorkbenchAgentStartProvider,
} from '@orchbee/workbench-client-protocol';

/** One Session Agent catalog as the Workbench reads it; the revision stays with the reader. */
export type WorkbenchAgentCatalogSnapshot = Omit<WorkbenchAgentCatalog, 'revision'>;

/** The Session's Agent catalog, read from the Server; product names and commands come from it. */
export interface WorkbenchAgentCatalogSource {
	/** Null until the first catalog arrives; the last catalog stays when the reading stops. */
	getSnapshot(): WorkbenchAgentCatalogSnapshot | null;
	subscribe(listener: () => void): () => void;
	/** Asks for a new lookup of installed Agents; its result arrives as a later catalog. */
	refresh(): void;
}

/** Where a started Agent opens, in the order the dialog offers them; New Tab is the default. */
export const WORKBENCH_AGENT_START_PLACES = [
	'new-tab',
	'split-right',
	'split-down',
	'new-worktree',
] as const;

export type WorkbenchAgentStartPlace = (typeof WORKBENCH_AGENT_START_PLACES)[number];

/** The Workspace row's target, held until the picker commits or closes. */
export interface WorkbenchAgentStartWorkspace {
	readonly id: string;
	readonly name: string;
}

/** Sidebar entry points backed by the Session's own Agent catalog. */
export interface WorkbenchWorkspaceAgentLaunch {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly open: (workspace: WorkbenchAgentStartWorkspace, returnTo: HTMLElement | null) => void;
	readonly start: (workspaceId: string, providerId: WorkbenchAgentStartProvider) => void;
}

/** One Agent to start and where it opens. */
export interface WorkbenchAgentStartInput {
	readonly place: WorkbenchAgentStartPlace;
	readonly providerId: WorkbenchAgentStartProvider;
	readonly workspaceId?: string;
}
