/** A Panel and the terminal this Client shows in it, which is a child terminal while one is displayed. */
export interface WorkbenchPanelTerminal {
	readonly panelId: string;
	readonly presentationId: string;
}

/**
 * The focused Panel of the active Tab and the Workspace holding it. A command that names no row reads
 * where this terminal stands, as the TUI's prefix commands read its focused Panel.
 */
export interface WorkbenchFocusedPanel extends WorkbenchPanelTerminal {
	readonly workspaceId: string;
}
