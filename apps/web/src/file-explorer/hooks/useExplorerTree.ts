import type { OrchbeeLocale } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import { ExplorerTree, type ExplorerTreeState } from '../model/ExplorerTree';
import { ExplorerViewMemory } from '../model/ExplorerViewMemory';

const LOADING: ExplorerTreeState = { kind: 'loading' };
/** Workspaces whose Explorer memory stays while the Explorer shows, least recently shown going. */
const REMEMBERED_WORKSPACES = 50;

function subscribeToNothing(): () => void {
	return () => undefined;
}

interface ExplorerTreeView {
	readonly state: ExplorerTreeState;
	/** The tree over the session; null without a session and before the first one is built. */
	readonly tree: ExplorerTree | null;
}

/**
 * The Explorer's tree over the session it is given. Another session or locale builds another tree,
 * and the one it replaces is disposed, releasing the folders it loaded; what each Workspace's
 * Explorer remembers stays for its next tree.
 */
export function useExplorerTree(
	session: FileWorkspaceSession | null,
	locale: OrchbeeLocale,
): ExplorerTreeView {
	const [memories] = React.useState(() => new Map<string, ExplorerViewMemory>());
	const [tree, setTree] = React.useState<ExplorerTree | null>(null);
	React.useEffect(() => {
		if (session === null) return undefined;
		const memory = memories.get(session.workspaceId) ?? new ExplorerViewMemory(null);
		memories.delete(session.workspaceId);
		memories.set(session.workspaceId, memory);
		for (const workspaceId of memories.keys()) {
			if (memories.size <= REMEMBERED_WORKSPACES) break;
			memories.delete(workspaceId);
		}
		const built = new ExplorerTree({ locale, memory, session });
		setTree(built);
		return () => {
			setTree(null);
			built.dispose();
		};
	}, [locale, memories, session]);
	const state = React.useSyncExternalStore(
		tree === null ? subscribeToNothing : tree.subscribe,
		() => (tree === null ? LOADING : tree.state),
	);
	return { state, tree };
}
