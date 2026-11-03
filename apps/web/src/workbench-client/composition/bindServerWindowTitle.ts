import type { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	workbenchSharedDocumentRevision,
	type TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import type { ServerConnection } from '../server-connection';
import {
	findServerPresentation,
	ServerWindowTitle,
	type ServerWorkbenchSession,
} from '../server-workbench';
import { resolveFocusedWorkbenchPresentationId } from './workbench-focus-recovery-storage';

/** A window's title and focus; the Server sends a title only to a focused or newly attached one. */
export interface ServerTitleWindow {
	readonly document: Pick<Document, 'hasFocus' | 'title'>;
	addEventListener(type: 'blur' | 'focus', listener: () => void): void;
	removeEventListener(type: 'blur' | 'focus', listener: () => void): void;
}

interface ServerWindowTitleBinding {
	/** Reads the context again after something it names changed outside a publication. */
	readonly refresh: () => void;
	/** Reads the context again when this terminal's title is the one it names. */
	readonly terminalTitleChanged: (presentationId: TerminalPresentationId) => void;
	dispose(): void;
}

/**
 * Keeps a window named after the Session's title policy, as the TUI keeps its outer terminal. It
 * listens with the window's focus and the TUI's display context: the active Workspace's and Tab's
 * labels as the Server holds them, the focused terminal's own label, and the title its program set.
 * It listens anew whenever any of them changes.
 */
export function bindServerWindowTitle({
	connection,
	runtime,
	session,
	terminalTitle,
	titleWindow,
}: {
	readonly connection: Pick<ServerConnection, 'subscribe'>;
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
	readonly session: Pick<ServerWorkbenchSession, 'snapshot' | 'subscribe'>;
	/** A terminal's program title as its chrome names it, while it comes from the terminal shown. */
	readonly terminalTitle: (presentationId: TerminalPresentationId) => string | null;
	readonly titleWindow: ServerTitleWindow;
}): ServerWindowTitleBinding {
	const windowTitle = new ServerWindowTitle(connection, session.snapshot.sessionId, (title) => {
		titleWindow.document.title = title;
	});
	let disposed = false;
	let namedPresentationId: TerminalPresentationId | null = null;
	const refresh = (): void => {
		if (disposed) return;
		const { snapshot } = session;
		const location = runtime.getSnapshot().activeLocation;
		const workspace =
			location.kind === 'workspace'
				? snapshot.navigation.workspaces.find(
						({ workspaceId }) => workspaceId === location.workspaceId,
					)
				: undefined;
		const context =
			workspace === undefined ? null : runtime.getTerminalContext(workspace.terminalContextId);
		const activeTabId = context?.tabModel.getSnapshot().activeTabId ?? null;
		const tab = snapshot.terminalContexts
			.find(({ terminalContextId }) => terminalContextId === workspace?.terminalContextId)
			?.tabs.find(({ tabId }) => tabId === activeTabId);
		const presentationId =
			context === null
				? null
				: resolveFocusedWorkbenchPresentationId(
						runtime.getSharedDocument(workbenchSharedDocumentRevision('0')),
						runtime.getClientView(),
					);
		namedPresentationId = presentationId;
		windowTitle.update(titleWindow.document.hasFocus(), {
			pane:
				(presentationId === null
					? null
					: findServerPresentation(snapshot, presentationId)?.customLabel) ?? '',
			tab: tab?.label ?? '',
			terminalTitle: (presentationId === null ? null : terminalTitle(presentationId)) ?? '',
			workspace: workspace?.label ?? '',
		});
	};
	const stopPublication = session.subscribe(refresh);
	const stopNavigation = runtime.subscribeNavigation(refresh);
	// Losing focus counts too: the Server brings a window forward only when its focus turns on.
	titleWindow.addEventListener('focus', refresh);
	titleWindow.addEventListener('blur', refresh);
	refresh();
	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			stopPublication();
			stopNavigation();
			titleWindow.removeEventListener('focus', refresh);
			titleWindow.removeEventListener('blur', refresh);
			windowTitle.dispose();
		},
		refresh,
		terminalTitleChanged: (presentationId) => {
			if (presentationId === namedPresentationId) refresh();
		},
	};
}
