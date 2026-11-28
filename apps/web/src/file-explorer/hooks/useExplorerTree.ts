import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileExplorerMessages, resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { FileError, FileWorkspaceSession } from '../../file-session';
import { ExplorerTree, type ExplorerTreeState } from '../model/ExplorerTree';
import { ExplorerViewMemory } from '../model/ExplorerViewMemory';
import type { FileExplorerNotifications } from '../types/file-explorer-notifications';
import { readFailureReason } from '../utils/readFailureReason';

const LOADING: ExplorerTreeState = { kind: 'loading' };
/** Workspaces whose Explorer memory stays while the Explorer shows, least recently shown going. */
const REMEMBERED_WORKSPACES = 50;

function subscribeToNothing(): () => void {
	return () => undefined;
}

interface ExplorerTreeOptions {
	readonly notifications: FileExplorerNotifications;
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
}

interface ExplorerTreeView {
	readonly state: ExplorerTreeState;
	/** The tree over the session; null without a session and before the first one is built. */
	readonly tree: ExplorerTree | null;
}

/**
 * The Explorer's tree over the session it is given. Another session or locale builds another tree,
 * and the one it replaces is disposed, releasing the folders it loaded; what each Workspace's
 * Explorer remembers stays for its next tree. A folder that could not be listed is reported once
 * while its notification shows, as VS Code shows one notification for repeats of the same message.
 */
export function useExplorerTree({ notifications, session }: ExplorerTreeOptions): ExplorerTreeView {
	const intl = useOrchbeeIntl();
	const locale = resolveOrchbeeLocale([intl.locale]);
	const [memories] = React.useState(() => new Map<string, ExplorerViewMemory>());
	const [shownReports] = React.useState(() => new Set<string>());
	const [tree, setTree] = React.useState<ExplorerTree | null>(null);
	const reportUnreadable = useMemoizedFn((name: string, error: FileError) => {
		const message = intl.formatMessage(fileExplorerMessages.folderUnreadable, {
			name,
			reason: intl.formatMessage(readFailureReason(error)),
		});
		if (shownReports.has(message)) return;
		shownReports.add(message);
		void notifications.notify({ message, severity: 'error' }).finally(() => {
			shownReports.delete(message);
		});
	});
	React.useEffect(() => {
		if (session === null) return undefined;
		const memory = memories.get(session.workspaceId) ?? new ExplorerViewMemory(null);
		memories.delete(session.workspaceId);
		memories.set(session.workspaceId, memory);
		for (const workspaceId of memories.keys()) {
			if (memories.size <= REMEMBERED_WORKSPACES) break;
			memories.delete(workspaceId);
		}
		const built = new ExplorerTree({ locale, memory, reportUnreadable, session });
		setTree(built);
		return () => {
			setTree(null);
			built.dispose();
		};
	}, [locale, memories, reportUnreadable, session]);
	const state = React.useSyncExternalStore(
		tree === null ? subscribeToNothing : tree.subscribe,
		() => (tree === null ? LOADING : tree.state),
	);
	return { state, tree };
}
