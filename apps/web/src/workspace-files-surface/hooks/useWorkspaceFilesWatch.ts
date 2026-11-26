import { useMemoizedFn } from '@orchbee/foundation/react';
import type { WorkspaceFilesWatchState } from '@orchbee/workbench-client-protocol';
import * as React from 'react';

import type { WorkbenchWorkspaceFilesSource, WorkspaceFilesWatchScope } from '../../workbench';

/** What may have changed: what shows in the tree and the open files, or the repository's status. */
export interface WorkspaceFilesChange {
	readonly files: boolean;
	readonly repository: boolean;
}

/** Changes are read this long after the last one, so a burst of writes reads once. */
const QUIET_MILLISECONDS = 1000;
/** A burst that never goes quiet is still read this often. */
const LONGEST_WAIT_MILLISECONDS = 5000;

const NONE: WorkspaceFilesChange = { files: false, repository: false };

function merge(left: WorkspaceFilesChange, right: WorkspaceFilesChange): WorkspaceFilesChange {
	return { files: left.files || right.files, repository: left.repository || right.repository };
}

export interface WorkspaceFilesWatchResult {
	/** The watch's state; null while nothing is watched. */
	readonly state: WorkspaceFilesWatchState | null;
	/** Reads a change now, without waiting for quiet, as when a terminal Agent stops working. */
	readonly notify: (change: WorkspaceFilesChange) => void;
}

/**
 * Follows `scope` in a Workspace while it is not null and calls `onChange` once the changes go quiet.
 * Nothing is read while the window is hidden; what changed meanwhile is read when it shows again.
 */
export function useWorkspaceFilesWatch(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string | null,
	scope: WorkspaceFilesWatchScope | null,
	onChange: (change: WorkspaceFilesChange) => void,
): WorkspaceFilesWatchResult {
	const [state, setState] = React.useState<WorkspaceFilesWatchState | null>(null);
	const change = useMemoizedFn(onChange);
	const pending = React.useRef<WorkspaceFilesChange>(NONE);
	const quiet = React.useRef<ReturnType<typeof setTimeout> | null>(null);
	const longest = React.useRef<ReturnType<typeof setTimeout> | null>(null);

	const cancelTimers = useMemoizedFn(() => {
		if (quiet.current !== null) clearTimeout(quiet.current);
		if (longest.current !== null) clearTimeout(longest.current);
		quiet.current = null;
		longest.current = null;
	});
	const flush = useMemoizedFn(() => {
		cancelTimers();
		if (document.hidden) return;
		const next = pending.current;
		pending.current = NONE;
		if (next.files || next.repository) change(next);
	});
	const notify = useMemoizedFn((next: WorkspaceFilesChange) => {
		pending.current = merge(pending.current, next);
		flush();
	});
	const record = useMemoizedFn((next: WorkspaceFilesChange) => {
		pending.current = merge(pending.current, next);
		if (quiet.current !== null) clearTimeout(quiet.current);
		quiet.current = setTimeout(flush, QUIET_MILLISECONDS);
		longest.current ??= setTimeout(flush, LONGEST_WAIT_MILLISECONDS);
	});

	React.useEffect(() => {
		const shown = (): void => {
			if (!document.hidden) flush();
		};
		document.addEventListener('visibilitychange', shown);
		return () => {
			document.removeEventListener('visibilitychange', shown);
			cancelTimers();
		};
	}, [cancelTimers, flush]);

	// The watch outlives a new scope, which it only updates, so it starts from the scope as it is.
	const watching = scope !== null && workspaceId !== null;
	const currentScope = useMemoizedFn(() => scope);
	const watch = React.useRef<ReturnType<WorkbenchWorkspaceFilesSource['watch']> | null>(null);
	React.useEffect(() => {
		const initial = currentScope();
		if (!watching || workspaceId === null || initial === null) return;
		const handle = source.watch(workspaceId, initial, (report) => {
			setState(report.state);
			if (report.files || report.repository) record(report);
		});
		watch.current = handle;
		return () => {
			handle.close();
			watch.current = null;
			pending.current = NONE;
			cancelTimers();
			setState(null);
		};
	}, [cancelTimers, currentScope, record, source, watching, workspaceId]);
	React.useEffect(() => {
		if (scope !== null) watch.current?.update(scope);
	}, [scope]);

	return { notify, state };
}
