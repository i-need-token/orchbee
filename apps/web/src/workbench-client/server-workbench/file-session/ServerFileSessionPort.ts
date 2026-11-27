import type { FileCapability, FileSessionPort, FileWorkspaceSession } from '../../../file-session';
import { ServerFileChannel } from './ServerFileChannel';
import { ServerFileSession } from './ServerFileSession';
import type { ServerFileConnection } from './server-file-exchange';

export interface ServerFileSessionOptions {
	/** The capabilities the Session's handshake negotiated; the File Bar's are kept. */
	readonly capabilities: readonly string[];
	readonly connection: ServerFileConnection;
	/** The machine that holds the Session's Workspaces, as the window names it. */
	readonly machine: string;
	/** A saved machine's Server, reached through the hub, which moves less data at once. */
	readonly remote: boolean;
}

const FILE_CAPABILITIES: readonly FileCapability[] = [
	'files',
	'files-edit',
	'files-history',
	'files-search',
];

/**
 * File sessions over one Server connection, for a runtime that shows that connection's Session.
 * Its sessions share the connection's room, loads and transfer IDs with any other runtime on the
 * same connection; disposing it closes the sessions it opened.
 */
export class ServerFileSessionPort implements FileSessionPort {
	readonly #capabilities: ReadonlySet<FileCapability>;
	readonly #channel: ServerFileChannel;
	readonly #machine: string;
	readonly #sessions = new Set<ServerFileSession>();

	constructor(options: ServerFileSessionOptions) {
		this.#capabilities = new Set(
			FILE_CAPABILITIES.filter((capability) => options.capabilities.includes(capability)),
		);
		this.#channel = ServerFileChannel.of(options.connection, options.remote);
		this.#machine = options.machine;
	}

	open(workspaceId: string): FileWorkspaceSession {
		const session: ServerFileSession = new ServerFileSession({
			capabilities: this.#capabilities,
			channel: this.#channel,
			machine: this.#machine,
			onClosed: () => this.#sessions.delete(session),
			workspaceId,
		});
		this.#sessions.add(session);
		return session;
	}

	dispose(): void {
		for (const session of this.#sessions) session.close();
	}
}
