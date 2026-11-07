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

interface ServerFocusNewPanelSourceOptions {
	readonly connection: Pick<ServerConnection, 'subscribe'>;
	readonly sessionId: string;
}

/**
 * Whether a Panel this window creates takes the focus, as the Session's configuration sets it. Each
 * state frame is a complete configuration snapshot, so the latest one answers without a round trip,
 * and the default holds until the first one arrives.
 */
export class ServerFocusNewPanelSource {
	readonly #connection: ServerFocusNewPanelSourceOptions['connection'];
	#focusNewPanel = true;
	#observation: ServerSubscription | null = null;

	constructor({ connection, sessionId }: ServerFocusNewPanelSourceOptions) {
		this.#connection = connection;
		this.#observe(sessionId);
	}

	/** The preference in force now. */
	current = (): boolean => this.#focusNewPanel;

	dispose(): void {
		this.#observation?.close();
		this.#observation = null;
	}

	#observe(sessionId: string): void {
		const requestId = createWorkbenchRequestId();
		let request: Uint8Array;
		try {
			request = encodeWorkbenchRequest(
				parseWorkbenchRequest({
					action: 'current',
					kind: 'workbench.configuration',
					protocolVersion: WORKBENCH_PROTOCOL_VERSION,
					requestId,
					sessionId,
				}),
			);
		} catch {
			return;
		}
		const observed = (record: Uint8Array): void => {
			let response: WorkbenchResponse;
			try {
				response = decodeWorkbenchResponse(record);
			} catch {
				return;
			}
			if (
				response.kind !== 'workbench.configuration.result' ||
				response.requestId !== requestId ||
				response.outcome.kind === 'failed'
			) {
				return;
			}
			this.#focusNewPanel = response.outcome.snapshot.focusNewPanel;
		};
		try {
			const observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					if (this.#observation === observation) this.#observation = null;
				},
				onFrame: (_kind, record) => {
					if (this.#observation === observation) observed(record);
				},
				// Every frame is a complete snapshot, so a gap loses nothing.
				onGap: () => undefined,
			});
			this.#observation = observation;
		} catch {
			// The connection has ended, which its Workbench session reports.
		}
	}
}
