import type { TerminalGeometry } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId, WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

import {
	type ServerConnection,
	type ServerHostColorsSource,
	type ServerTerminalBinding,
	type ServerTerminalProjection,
	ServerTerminalProjectionError,
	type ServerTerminalSettlement,
	openServerTerminalProjection,
} from '../server-connection';
import type { TerminalProjectionClient, TerminalProjectionRequest } from '../terminal-projection';
import { findServerTerminalBinding, serverTerminalBindingKey } from './server-terminal-binding';

/** Hears how the Host settled the terminal a Presentation was bound to when its projection opened. */
export type ServerTerminalSettlementObserver = (
	presentationId: TerminalPresentationId,
	binding: ServerTerminalBinding,
	settlement: ServerTerminalSettlement,
) => void;

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
 * and while it finishes opening, so the opens of one terminal wait for both. Each end a projection
 * sees reaches `onSettled` with the binding it opened, which the Presentation may have left since.
 */
export class ServerTerminalProjectionClient implements TerminalProjectionClient {
	readonly #closing = new Map<string, Promise<void>>();
	readonly #connection: ServerConnection;
	readonly #hostColors: ServerHostColorsSource | undefined;
	readonly #onSettled: ServerTerminalSettlementObserver | undefined;
	readonly #readSnapshot: () => WorkbenchSnapshot;
	readonly #releases = new Set<() => void>();
	readonly #tails = new Map<string, Promise<void>>();
	readonly #opened = new Map<TerminalPresentationId, ServerTerminalProjection>();
	#focus: {
		id: TerminalPresentationId;
		initial: boolean;
		signal: AbortSignal;
		started: boolean;
	} | null = null;

	activateFocus(id: TerminalPresentationId, initial: boolean, signal: AbortSignal): void {
		this.#focus = { id, initial, signal, started: false };
		this.#driveFocus();
	}
	#driveFocus(): void {
		const focus = this.#focus;
		if (focus === null || focus.started || focus.signal.aborted) return;
		const projection = this.#opened.get(focus.id);
		if (projection === undefined) return;
		focus.started = true;
		void projection.activateFocus(focus.initial, focus.signal);
	}

	#disposed = false;

	constructor(
		connection: ServerConnection,
		readSnapshot: () => WorkbenchSnapshot,
		hostColors?: ServerHostColorsSource,
		onSettled?: ServerTerminalSettlementObserver,
	) {
		this.#connection = connection;
		this.#hostColors = hostColors;
		this.#onSettled = onSettled;
		this.#readSnapshot = readSnapshot;
	}

	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
	): TerminalProjectionRequest {
		if (this.#disposed) return refusedRequest('terminal-projection-unavailable', true);
		const binding = findServerTerminalBinding(this.#readSnapshot(), presentationId);
		if (binding === null) return refusedRequest('terminal-presentation-not-ready', true);

		const key = serverTerminalBindingKey(binding);
		let projection: ServerTerminalProjection | null = null;
		let released = false;
		const release = (): void => {
			released = true;
			this.#releases.delete(release);
			if (projection === null) return;
			if (this.#opened.get(presentationId) === projection) this.#opened.delete(presentationId);
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
			const opened = await openServerTerminalProjection(
				this.#connection,
				binding,
				geometry,
				this.#hostColors,
				(settlement) => this.#onSettled?.(presentationId, binding, settlement),
			);
			if (released) {
				await opened.close();
				throw new ServerTerminalProjectionError('terminal-projection-released', false);
			}
			projection = opened;
			this.#opened.set(presentationId, opened);
			this.#driveFocus();
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
