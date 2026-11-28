/**
 * What a multi-diff's owner does with the list: VS Code's Collapse All Diffs and Expand All Diffs,
 * whose title action shows Expand All while every file is collapsed.
 */
export interface CodeEditorMultiDiffHandle {
	/** Whether every file is collapsed; with no files, as in VS Code, they all count as collapsed. */
	allCollapsed(): boolean;
	collapseAll(): void;
	expandAll(): void;
	/** Calls back after any change to the list, such as one to `allCollapsed`. */
	subscribe(listener: () => void): () => void;
}
