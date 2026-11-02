import type { TerminalGeometry } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import {
	type ServerConnection,
	type ServerTerminalProjection,
	ServerTerminalProjectionError,
	openServerTerminalProjection,
} from '../server-connection';
import type { TerminalProjectionClient, TerminalProjectionRequest } from '../terminal-projection';
import { findServerTerminalBinding } from './findServerTerminalBinding';

function refusedRequest(code: string, retryable: boolean): TerminalProjectionRequest {
	return {
		release: () => undefined,
		result: Promise.reject(new ServerTerminalProjectionError(code, retryable)),
	};
}

/**
 * Projects whichever terminal a Presentation is bound to when a surface asks. A surface being
 * replaced releases its projection before its successor opens, and closing a lane releases control
 * before the Server reads the next request, so a layout change here never meets its own control.
 * A projection holds its lane past release in two cases, while it delivers what was typed into it
 * and while it finishes opening, so the opens of one terminal wait for both.
 */
export class ServerTerminalProjectionClient implements TerminalProjectionClient {
	readonly #closing = new Map<string, Promise<void>>();
	readonly #connection: ServerConnection;
	readonly #readSnapshot: () => WorkbenchSnapshot;
	readonly #releases = new Set<() => void>();
	readonly #tails = new Map<string, Promise<void>>();
	#disposed = false;

	constructor(connection: ServerConnection, readSnapshot: () => WorkbenchSnapshot) {
		this.#connection = connection;
		this.#readSnapshot = readSnapshot;
	}

	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
	): TerminalProjectionRequest {
		if (this.#disposed) return refusedRequest('terminal-projection-unavailable', true);
		const binding = findServerTerminalBinding(this.#readSnapshot(), presentationId);
		if (binding === null) return refusedRequest('terminal-presentation-not-ready', true);

		const key = `${binding.workerId}:${binding.terminalId}`;
		let projection: ServerTerminalProjection | null = null;
		let released = false;
		const release = (): void => {
			released = true;
			this.#releases.delete(release);
			if (projection === null) return;
			const closing = projection.close();
			this.#closing.set(key, closing);
			void closing.then(() => {
				if (this.#closing.get(key) === closing) this.#closing.delete(key);
			});
		};
		this.#releases.add(release);

		const result = (this.#tails.get(key) ?? Promise.resolve()).then(async () => {
			// Read only now: the surface this one replaces may release in the same commit, after this.
			await this.#closing.get(key);
			if (released) throw new ServerTerminalProjectionError('terminal-projection-released', false);
			const opened = await openServerTerminalProjection(this.#connection, binding, geometry);
			if (released) {
				await opened.close();
				throw new ServerTerminalProjectionError('terminal-projection-released', false);
			}
			projection = opened;
			return opened;
		});
		const settled = result.then(
			() => undefined,
			() => undefined,
		);
		this.#tails.set(key, settled);
		void settled.then(() => {
			if (this.#tails.get(key) === settled) this.#tails.delete(key);
		});
		return { release, result };
	}

	dispose(): void {
		this.#disposed = true;
		for (const release of this.#releases) release();
	}
}
