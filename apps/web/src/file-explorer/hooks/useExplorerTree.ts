import type { OrchbeeLocale } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../../file-session';
import { ExplorerTree, type ExplorerTreeState } from '../model/ExplorerTree';

const LOADING: ExplorerTreeState = { kind: 'loading' };

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
 * and the one it replaces is disposed, releasing the folders it loaded.
 */
export function useExplorerTree(
	session: FileWorkspaceSession | null,
	locale: OrchbeeLocale,
): ExplorerTreeView {
	const [tree, setTree] = React.useState<ExplorerTree | null>(null);
	React.useEffect(() => {
		if (session === null) return undefined;
		const built = new ExplorerTree({ locale, session });
		setTree(built);
		return () => {
			setTree(null);
			built.dispose();
		};
	}, [locale, session]);
	const state = React.useSyncExternalStore(
		tree === null ? subscribeToNothing : tree.subscribe,
		() => (tree === null ? LOADING : tree.state),
	);
	return { state, tree };
}
