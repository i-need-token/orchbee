import { createStore, type StoreApi } from 'zustand/vanilla';

type WorkbenchPane = 'primary' | 'secondary';
export type WorkbenchSplitOrientation = 'horizontal' | 'vertical';

interface WorkbenchLayoutState {
	readonly activeSessionId: string | undefined;
	readonly splitSessionId: string | undefined;
	readonly splitOrientation: WorkbenchSplitOrientation;
	readonly focusedPane: WorkbenchPane;
	selectSession(sessionId: string): void;
	splitWithSession(sessionId: string, orientation: WorkbenchSplitOrientation): void;
	closeSplit(): void;
	focusPane(pane: WorkbenchPane): void;
	reconcileSessions(sessionIds: readonly string[]): void;
}

export type WorkbenchLayoutStore = StoreApi<WorkbenchLayoutState>;

export function createWorkbenchLayoutStore(
	initialSessionId: string | undefined,
): WorkbenchLayoutStore {
	return createStore<WorkbenchLayoutState>()((set) => ({
		activeSessionId: initialSessionId,
		splitSessionId: undefined,
		splitOrientation: 'horizontal',
		focusedPane: 'primary',
		selectSession: (sessionId) => set({ activeSessionId: sessionId, focusedPane: 'primary' }),
		splitWithSession: (sessionId, splitOrientation) =>
			set({ splitSessionId: sessionId, splitOrientation, focusedPane: 'secondary' }),
		closeSplit: () => set({ splitSessionId: undefined, focusedPane: 'primary' }),
		focusPane: (focusedPane) => set({ focusedPane }),
		reconcileSessions: (sessionIds) =>
			set((state) => {
				const sessionIdSet = new Set(sessionIds);
				const activeSessionId =
					state.activeSessionId !== undefined && sessionIdSet.has(state.activeSessionId)
						? state.activeSessionId
						: sessionIds[0];
				const splitSessionId =
					state.splitSessionId !== undefined && sessionIdSet.has(state.splitSessionId)
						? state.splitSessionId
						: undefined;
				const focusedPane = splitSessionId === undefined ? 'primary' : state.focusedPane;
				return { activeSessionId, splitSessionId, focusedPane };
			}),
	}));
}
