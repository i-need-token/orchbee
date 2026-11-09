import type { ActionResult } from '@orchbee/actions';

/**
 * Why the Server would not use a folder named for a new Workspace: the Worker's cause, or
 * `directory-unavailable` when it names none. The Client never probes the path itself, since the
 * Worker may be on another machine.
 */
export type WorkbenchDirectoryRefusal =
	| 'directory-inaccessible'
	| 'directory-missing'
	| 'directory-unavailable'
	| 'not-a-directory';

/** The folder refusal a creation's result carries, or null for any other outcome. */
export function readWorkbenchDirectoryRefusal(
	result: ActionResult<unknown>,
): WorkbenchDirectoryRefusal | null {
	if (result.kind !== 'failed') return null;
	switch (result.failure.code) {
		case 'directory-inaccessible':
			return 'directory-inaccessible';
		case 'directory-missing':
			return 'directory-missing';
		case 'directory-unavailable':
			return 'directory-unavailable';
		case 'not-a-directory':
			return 'not-a-directory';
		default:
			return null;
	}
}
