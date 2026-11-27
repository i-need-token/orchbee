import { describe, expect, it } from 'vitest';

import { FileSessionRegistry } from '../FileSessionRegistry';
import type { FileSessionPort, FileWorkspaceSession } from '../file-session-port';

/** A port whose sessions only record that they are open. */
function recordingPort() {
	const open: string[] = [];
	const port: FileSessionPort = {
		open: (workspaceId) => {
			open.push(workspaceId);
			const session: Pick<FileWorkspaceSession, 'close' | 'workspaceId'> = {
				close: () => {
					open.splice(open.indexOf(workspaceId), 1);
				},
				workspaceId,
			};
			return session as FileWorkspaceSession;
		},
	};
	return { open, port };
}

describe('the file session registry', () => {
	it('shares one session per Workspace among its holds and closes it with the last release', () => {
		const { open, port } = recordingPort();
		const registry = new FileSessionRegistry(port);

		const bar = registry.hold('workspace-1');
		const overlay = registry.hold('workspace-1');
		const other = registry.hold('workspace-2');
		expect(overlay.session).toBe(bar.session);
		expect(open).toEqual(['workspace-1', 'workspace-2']);

		bar.release();
		bar.release();
		expect(open).toEqual(['workspace-1', 'workspace-2']);
		overlay.release();
		expect(open).toEqual(['workspace-2']);

		// A later hold opens a fresh session.
		expect(registry.hold('workspace-1').session).not.toBe(bar.session);
		other.release();
		expect(open).toEqual(['workspace-1']);
	});
});
