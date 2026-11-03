/** What Git reported for the directory a Workspace's location names. */
export interface WorkbenchWorkspaceGit {
	/** The checked-out branch, or null when HEAD is detached or the directory is no repository. */
	readonly branch: string | null;
	/** Commits to push and to pull against the branch's upstream, or null without one. */
	readonly divergence: { readonly ahead: number; readonly behind: number } | null;
}

/** Where a Workspace stands, as its sidebar row shows it. */
export interface WorkbenchWorkspaceLocation {
	/**
	 * The first terminal's directory, otherwise the Workspace's own, with a local home folder shortened
	 * to `~`; null when neither is known.
	 */
	readonly folder: string | null;
	/** Null until Git answers for that directory. */
	readonly git: WorkbenchWorkspaceGit | null;
	/**
	 * The name the row keeps after its directory moved, until Git answers for the new one, so the row
	 * changes once; null shows the Workspace's own name.
	 */
	readonly heldName: string | null;
}

/** The locations of the Workspaces a Client shows, observed only while someone subscribes. */
export interface WorkbenchWorkspaceLocationSource {
	/** The latest locations by Workspace ID; the same object until one changes. */
	getSnapshot(): ReadonlyMap<string, WorkbenchWorkspaceLocation>;
	subscribe(listener: () => void): () => void;
}
