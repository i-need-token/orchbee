import * as React from 'react';

import type { WorkbenchSession, WorkbenchSessionService } from '../../sessions';
import {
	createWorkbenchLayoutStore,
	type WorkbenchSplitOrientation,
} from '../contexts/createWorkbenchLayoutStore';

interface UseWorkbenchResult {
	readonly activeSession: WorkbenchSession | undefined;
	readonly activeSessionId: string | undefined;
	readonly closeActiveSession: () => void;
	readonly closeSplit: () => void;
	readonly createSession: () => void;
	readonly focusPane: (pane: 'primary' | 'secondary') => void;
	readonly focusedPane: 'primary' | 'secondary';
	readonly selectSession: (sessionId: string) => void;
	readonly sessions: readonly WorkbenchSession[];
	readonly split: (orientation: WorkbenchSplitOrientation) => void;
	readonly splitOrientation: WorkbenchSplitOrientation;
	readonly splitSession: WorkbenchSession | undefined;
}

export function useWorkbench(sessionService: WorkbenchSessionService): UseWorkbenchResult {
	const snapshot = React.useSyncExternalStore(
		(listener) => sessionService.subscribe(listener),
		() => sessionService.getSnapshot(),
		() => sessionService.getSnapshot(),
	);
	const [layoutStore] = React.useState(() => createWorkbenchLayoutStore(snapshot.sessions[0]?.id));
	const layout = React.useSyncExternalStore(
		(listener) => layoutStore.subscribe(listener),
		() => layoutStore.getState(),
		() => layoutStore.getState(),
	);

	React.useEffect(() => {
		layoutStore.getState().reconcileSessions(snapshot.sessions.map((session) => session.id));
	}, [layoutStore, snapshot]);

	const activeSession = snapshot.sessions.find((session) => session.id === layout.activeSessionId);
	const splitSession = snapshot.sessions.find((session) => session.id === layout.splitSessionId);

	return {
		activeSession,
		activeSessionId: layout.activeSessionId,
		closeActiveSession: () => {
			if (activeSession !== undefined) sessionService.closeSession(activeSession.id);
		},
		closeSplit: () => layoutStore.getState().closeSplit(),
		createSession: () => {
			const sessionId = sessionService.createSession();
			layoutStore.getState().selectSession(sessionId);
		},
		focusPane: (pane) => layoutStore.getState().focusPane(pane),
		focusedPane: layout.focusedPane,
		selectSession: (sessionId) => layoutStore.getState().selectSession(sessionId),
		sessions: snapshot.sessions,
		split: (orientation) => {
			const splitSessionId = snapshot.sessions.find(
				(session) => session.id !== layout.activeSessionId,
			)?.id;
			layoutStore
				.getState()
				.splitWithSession(splitSessionId ?? sessionService.createSession(), orientation);
		},
		splitOrientation: layout.splitOrientation,
		splitSession,
	};
}
