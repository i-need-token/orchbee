import type { FileSessionPort, FileWorkspaceSession } from './file-session-port';

/** A hold on one Workspace's session, which stays open while any hold does. */
interface FileSessionHold {
	readonly session: FileWorkspaceSession;
	/** Ends this hold; releasing it again does nothing. */
	release(): void;
}

interface HeldSession {
	holders: number;
	readonly session: FileWorkspaceSession;
}

/**
 * The Client's file sessions, one per Workspace. The first hold opens a Workspace's session and the
 * last release closes it, so a Workspace whose files nothing shows has no session.
 */
export class FileSessionRegistry {
	private readonly held = new Map<string, HeldSession>();
	private readonly port: FileSessionPort;

	public constructor(port: FileSessionPort) {
		this.port = port;
	}

	public hold(workspaceId: string): FileSessionHold {
		let entry = this.held.get(workspaceId);
		if (entry === undefined) {
			entry = { holders: 0, session: this.port.open(workspaceId) };
			this.held.set(workspaceId, entry);
		}
		entry.holders += 1;
		const held = entry;
		let released = false;
		return {
			release: () => {
				if (released) return;
				released = true;
				held.holders -= 1;
				if (held.holders > 0) return;
				this.held.delete(workspaceId);
				held.session.close();
			},
			session: held.session,
		};
	}
}
