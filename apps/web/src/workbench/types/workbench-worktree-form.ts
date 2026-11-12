/** What the New Worktree form submitted; a field left blank is null and takes its default. */
export interface WorkbenchWorktreeFormAnswer {
	/** The branch, tag or commit to start from; null starts from HEAD. */
	readonly base: string | null;
	/** The branch to create; null generates one and gives the Workspace a default name. */
	readonly branch: string | null;
}

/** What the form shows when it opens. */
interface WorkbenchWorktreeFormPrefill {
	/** Blank for the default, or the branch Start Agent proposes for its Agent. */
	readonly branch: string;
}

/**
 * Opens the New Worktree form and settles with what it submitted, or with null once the person
 * cancels it or `signal` aborts, which also closes it. Nothing is created before it settles.
 */
export type WorkbenchWorktreeFormRequest = (
	prefill: WorkbenchWorktreeFormPrefill,
	signal: AbortSignal,
) => Promise<WorkbenchWorktreeFormAnswer | null>;
