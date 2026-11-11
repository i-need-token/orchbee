import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type WorkbenchAgentCatalog,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchAgentCatalogSource } from '../../workbench';
import {
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
	type ServerConnection,
	type ServerSubscription,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

interface ServerAgentCatalogOptions {
	readonly connection: Pick<ServerConnection, 'exchange' | 'subscribe'>;
	readonly sessionId: string;
}

/**
 * The Session's Agent catalog as the Server publishes it, read over one State stream that this
 * window holds while it is connected. Every frame is a whole catalog, parsed by the protocol's
 * response union; a later revision replaces the one shown, and a failed or closed stream keeps the
 * last catalog. Nothing polls: `refresh` asks the Server for a new lookup, whose rows arrive as
 * frames.
 */
export class ServerAgentCatalog implements WorkbenchAgentCatalogSource {
	readonly #connection: ServerAgentCatalogOptions['connection'];
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#catalog: WorkbenchAgentCatalog | null = null;
	#disposed = false;
	#observation: ServerSubscription | null = null;

	constructor({ connection, sessionId }: ServerAgentCatalogOptions) {
		this.#connection = connection;
		this.#sessionId = sessionId;
		this.#observe();
	}

	getSnapshot = (): WorkbenchAgentCatalog | null => this.#catalog;

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	refresh = (): void => {
		if (this.#disposed) return;
		const requestId = createWorkbenchRequestId();
		const request = this.#encode(requestId, 'refresh');
		if (request === null) return;
		// The answer holds the catalog as it stands; the lookup it starts arrives on the stream.
		this.#connection.exchange(request).then(
			(record) => this.#adopt(record, requestId),
			() => undefined,
		);
	};

	dispose(): void {
		this.#disposed = true;
		this.#observation?.close();
		this.#observation = null;
		this.#listeners.clear();
	}

	#encode(requestId: string, action: 'current' | 'refresh'): Uint8Array | null {
		try {
			return encodeWorkbenchRequest(
				parseWorkbenchRequest({
					action: { kind: action },
					kind: 'workbench.agent-catalog',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId: this.#sessionId,
				}),
			);
		} catch {
			return null;
		}
	}

	/** Keeps the shown catalog unless `record` carries a later revision of it. */
	#adopt(record: Uint8Array, requestId: string): void {
		if (this.#disposed) return;
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (
			response.kind !== 'workbench.agent-catalog.result' ||
			response.requestId !== requestId ||
			response.outcome.kind !== 'current'
		) {
			return;
		}
		const { catalog } = response.outcome;
		if (this.#catalog !== null && BigInt(catalog.revision) <= BigInt(this.#catalog.revision)) {
			return;
		}
		this.#catalog = catalog;
		for (const listener of this.#listeners) listener();
	}

	#observe(): void {
		const requestId = createWorkbenchRequestId();
		const request = this.#encode(requestId, 'current');
		if (request === null) return;
		try {
			const observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					if (this.#observation === observation) this.#observation = null;
				},
				onFrame: (_kind, record) => {
					if (this.#observation === observation) this.#adopt(record, requestId);
				},
				// Every frame is a whole catalog, so a gap loses nothing.
				onGap: () => undefined,
			});
			this.#observation = observation;
		} catch {
			// The connection has ended, which its Workbench session reports.
		}
	}
}
