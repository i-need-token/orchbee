import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type TerminalRuntimeOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.terminal-runtime.result' }
>['outcome'];
/** `serving`, `reconnecting` while the Server replaces a lost Worker, or `given-up`. */
export type ServerTerminalRuntimeState = Extract<
	TerminalRuntimeOutcome,
	{ readonly kind: 'current' }
>['runtime']['state'];

/** A stream the Server closed opens again after this long. */
const REOPEN_MILLISECONDS = 2000;

interface ServerTerminalRuntimeOptions {
	readonly connection: Pick<ServerConnection, 'subscribe'>;
	readonly sessionId: string;
}

/**
 * The Session's terminal-runtime state, read over one `workbench.terminal-runtime` State stream that
 * this window holds while it is connected; only a Server whose handshake lists the capability is
 * asked. A later revision replaces the state shown. While no stream answers, the state is `serving`,
 * so each terminal keeps its own recovery as it would without the read.
 */
export class ServerTerminalRuntime {
	readonly #connection: ServerTerminalRuntimeOptions['connection'];
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#observation: ServerSubscription | null = null;
	#reopen: ReturnType<typeof setTimeout> | null = null;
	/** The revision the open stream last answered with; a reopened stream starts again. */
	#revision: bigint | null = null;
	#state: ServerTerminalRuntimeState = 'serving';

	constructor({ connection, sessionId }: ServerTerminalRuntimeOptions) {
		this.#connection = connection;
		this.#sessionId = sessionId;
		this.#observe();
	}

	getSnapshot = (): ServerTerminalRuntimeState => this.#state;

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	dispose(): void {
		this.#disposed = true;
		if (this.#reopen !== null) clearTimeout(this.#reopen);
		this.#reopen = null;
		this.#observation?.close();
		this.#observation = null;
		this.#listeners.clear();
	}

	#set(state: ServerTerminalRuntimeState): void {
		if (this.#disposed || state === this.#state) return;
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}

	#adopt(record: Uint8Array, requestId: string): void {
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (response.kind !== 'workbench.terminal-runtime.result' || response.requestId !== requestId) {
			return;
		}
		if (response.outcome.kind !== 'current') {
			this.#set('serving');
			return;
		}
		const { revision, state } = response.outcome.runtime;
		if (this.#revision !== null && BigInt(revision) <= this.#revision) return;
		this.#revision = BigInt(revision);
		this.#set(state);
	}

	#observe(): void {
		if (this.#disposed) return;
		const requestId = createWorkbenchRequestId();
		let request: Uint8Array;
		try {
			request = encodeWorkbenchRequest(
				parseWorkbenchRequest({
					kind: 'workbench.terminal-runtime',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: this.#sessionId,
				}),
			);
		} catch {
			return;
		}
		this.#revision = null;
		try {
			const observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					if (this.#observation !== observation) return;
					this.#observation = null;
					this.#set('serving');
					this.#reopen = setTimeout(() => {
						this.#reopen = null;
						this.#observe();
					}, REOPEN_MILLISECONDS);
				},
				onFrame: (_kind, record) => {
					if (this.#observation === observation) this.#adopt(record, requestId);
				},
				// Every frame is the whole state, so a gap loses nothing.
				onGap: () => undefined,
			});
			this.#observation = observation;
		} catch {
			// The connection has ended, which its Workbench session reports.
		}
	}
}
