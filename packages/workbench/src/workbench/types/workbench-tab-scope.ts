export interface WorkbenchTabScopeTerminalContext {
	readonly tabIds: readonly string[];
	readonly terminalContextId: string;
}

export interface WorkbenchTabScopeSnapshot {
	readonly revision: string;
	readonly terminalContexts: readonly WorkbenchTabScopeTerminalContext[];
}

/** Host-owned projection of the shared Workbench tabs visible in this view. */
export interface WorkbenchTabScopeSource {
	readonly getSnapshot: () => WorkbenchTabScopeSnapshot;
	readonly subscribe: (listener: () => void) => () => void;
}
