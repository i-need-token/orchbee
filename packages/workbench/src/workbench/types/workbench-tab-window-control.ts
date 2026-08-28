import type { WorkbenchTabScopeSnapshot, WorkbenchTabScopeSource } from './workbench-tab-scope';

export type WorkbenchTabWindowDestination =
	| { readonly kind: 'existing'; readonly windowId: string }
	| { readonly kind: 'new' };

export type WorkbenchTabWindowMoveResult =
	| { readonly destinationWindowId: string; readonly kind: 'completed' }
	| { readonly kind: 'unchanged' }
	| { readonly code: string; readonly kind: 'failed' };

export interface WorkbenchTabWindowSummary {
	readonly ordinal: number;
	readonly windowId: string;
}

export interface WorkbenchTabWindowSnapshot extends WorkbenchTabScopeSnapshot {
	readonly currentWindowId: string;
	readonly windows: readonly WorkbenchTabWindowSummary[];
}

/** Native host capability for moving shared tabs without restarting their terminals. */
export interface WorkbenchTabWindowControl extends WorkbenchTabScopeSource {
	readonly getSnapshot: () => WorkbenchTabWindowSnapshot;
	finishDrag(
		sessionId: string,
		input:
			| { readonly reason: 'cancel' }
			| {
					readonly point: Readonly<{ x: number; y: number }>;
					readonly reason: 'complete';
			  },
	): Promise<WorkbenchTabWindowMoveResult>;
	getDragTarget(sessionId: string): WorkbenchTabWindowDestination | null;
	moveGroup(
		terminalContextId: string,
		groupId: string,
		destination: WorkbenchTabWindowDestination,
	): Promise<WorkbenchTabWindowMoveResult>;
	moveTabs(
		terminalContextId: string,
		tabIds: readonly string[],
		destination: WorkbenchTabWindowDestination,
	): Promise<WorkbenchTabWindowMoveResult>;
	startDrag(terminalContextId: string, tabIds: readonly string[]): string | null;
	updateDrag(sessionId: string, point: Readonly<{ x: number; y: number }>): void;
}
