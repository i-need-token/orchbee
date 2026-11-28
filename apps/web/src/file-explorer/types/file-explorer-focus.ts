/** How the File Bar's column moves focus into the Explorer. */
export interface FileExplorerFocus {
	/** Focuses the tree, as VS Code's Explorer does when its view is focused. */
	focus(): void;
}
