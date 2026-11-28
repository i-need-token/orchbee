import * as React from 'react';

import type { GitPort } from '../../file-session';
import { sourceControlCount } from '../model/sourceControlCount';
import { useGitClientFocus } from './useGitClientFocus';

interface CountStore {
	readonly read: () => number | null;
	readonly subscribe: (listener: () => void) => () => void;
}

const NO_COUNT: CountStore = { read: () => null, subscribe: () => () => undefined };

/** Follows a session's summary, and nothing else, while someone shows its count. */
function createCountStore(git: GitPort): CountStore {
	let count: number | null = null;
	return {
		read: () => count,
		subscribe: (listener) =>
			git.subscribe((event) => {
				if (event.kind !== 'summary') return;
				count = sourceControlCount(event.summary);
				listener();
			}),
	};
}

/**
 * The Source Control count badge for a session: its pending changes, or null before its first
 * summary and without a session. It reads only the summary, never a status page, so a badge costs
 * the Workspace nothing beyond the stream that also feeds the Explorer's decorations. Like any
 * follower of the summary, it tells the backend whether the window has focus, which the automatic
 * refresh after a file change waits for, so the count follows changes while the view is hidden.
 */
export function useSourceControlCount(git: GitPort | null): number | null {
	useGitClientFocus(git);
	const store = React.useMemo(() => (git === null ? NO_COUNT : createCountStore(git)), [git]);
	return React.useSyncExternalStore(store.subscribe, store.read);
}
