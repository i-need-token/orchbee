import type { ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';

import type { FileOverlayOpenFile } from '../../file-overlay';
import {
	filePathKey,
	type FilePathRules,
	type FileSessionRegistry,
	type FileWorkspaceSession,
} from '../../file-session';
import type {
	WorkbenchTerminalFileOpener,
	WorkbenchTerminalFileOpenResult,
	WorkbenchTerminalFileTarget,
} from '../types/workbench-terminal-surface';

interface UseWorkbenchTerminalFileLinksOptions {
	/** The Workbench's own scope, where the file overlay's open Action resolves. */
	readonly actionScope: ActionScope;
	/** Whether this window offers the File Bar; without the offer nothing opens terminal file links. */
	readonly offered: boolean;
	/** Opens a file in the shown Workspace's overlay. */
	readonly openFile: FileOverlayOpenFile;
	/** The Client's file sessions, which state a link's path; without them no link opens a file. */
	readonly sessions: FileSessionRegistry | null;
	/** The active Workspace, whose terminals the terminal area shows; null outside a Workspace. */
	readonly workspaceId: string | null;
}

const CANCELLED: WorkbenchTerminalFileOpenResult = { kind: 'cancelled' };
const COMPLETED: WorkbenchTerminalFileOpenResult = { kind: 'completed' };
/** How long a stated link waits for its session's path rules, which its root stream sends first. */
const RULES_WAIT_MS = 10_000;

function failed(code: string): WorkbenchTerminalFileOpenResult {
	return { code, kind: 'failed', retryable: false };
}

/** The session's path rules, from its root stream's snapshot; null once the signal or the wait ends. */
function readRules(
	session: FileWorkspaceSession,
	signal: AbortSignal,
): Promise<FilePathRules | null> {
	return new Promise((resolve) => {
		if (signal.aborted) {
			resolve(null);
			return;
		}
		let settled = false;
		let timer: ReturnType<typeof setTimeout> | undefined;
		let unsubscribe: (() => void) | undefined;
		const settle = (rules: FilePathRules | null): void => {
			if (settled) return;
			settled = true;
			clearTimeout(timer);
			signal.removeEventListener('abort', abandon);
			unsubscribe?.();
			resolve(rules);
		};
		const abandon = (): void => settle(null);
		timer = setTimeout(abandon, RULES_WAIT_MS);
		signal.addEventListener('abort', abandon, { once: true });
		const stop = session.subscribe((event) => {
			if (event.kind === 'snapshot') settle(event.rules);
		});
		if (settled) stop();
		else unsubscribe = stop;
	});
}

/**
 * Terminal file links in a window that offers the File Bar, as VS Code's terminal opens a link to a
 * local file: the path a followed link names is stated in the active Workspace's file session, and
 * a file, or a symbolic link to one, opens in the file overlay as a pinned tab that takes the keys. A
 * missing path, one outside the Workspace's boundary, a folder and anything else open nothing and
 * fail with the File Bar's code for why. The overlay's tabs carry no position yet, so the link's line
 * and column go unused.
 */
export function useWorkbenchTerminalFileLinks({
	actionScope,
	offered,
	openFile,
	sessions,
	workspaceId,
}: UseWorkbenchTerminalFileLinksOptions): WorkbenchTerminalFileOpener | undefined {
	const open = useMemoizedFn(
		async (
			target: WorkbenchTerminalFileTarget,
			signal: AbortSignal,
		): Promise<WorkbenchTerminalFileOpenResult> => {
			// A link is followed in a shown terminal, which the active Workspace holds.
			const opening = workspaceId;
			if (sessions === null || opening === null) return failed('no-workspace');
			const hold = sessions.hold(opening);
			try {
				const stated = await hold.session.tree.stat([target.path], signal);
				if (signal.aborted) return CANCELLED;
				if (stated.kind === 'error') return failed(stated.error.code);
				const item = stated.value[0];
				if (item === undefined) return failed('failed');
				if (item.kind === 'error') return failed(item.error.code);
				if (item.entry.type === 'directory') return failed('is-directory');
				if (item.entry.type !== 'file') return failed('not-file');
				const rules = await readRules(hold.session, signal);
				if (signal.aborted) return CANCELLED;
				if (rules === null) return failed('timeout');
				const result = await openFile(
					{
						focus: true,
						preview: false,
						resource: { key: filePathKey(item.path, rules), path: item.path },
						workspaceId: opening,
					},
					// The terminal's Action turned the click into the open, so it runs the overlay's.
					{ scope: actionScope, source: 'programmatic' },
				).completion;
				switch (result.kind) {
					case 'completed':
					case 'unchanged':
						return COMPLETED;
					case 'cancelled':
						return CANCELLED;
					case 'blocked':
						return failed(result.code);
					case 'failed':
						return failed(result.failure.code);
					case 'unsupported':
						return failed('unsupported');
				}
			} finally {
				hold.release();
			}
		},
	);
	return offered && sessions !== null ? open : undefined;
}
