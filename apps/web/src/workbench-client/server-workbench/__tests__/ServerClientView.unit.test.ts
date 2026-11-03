import { parseWorkbenchRequest, type WorkbenchRequest } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import type { ServerConnection } from '../../server-connection';
import { ServerClientView, type ServerClientViewSample } from '../ServerClientView';

type Handlers = Parameters<ServerConnection['subscribe']>[2];
type ClientViewRequest = Extract<WorkbenchRequest, { readonly kind: 'workbench.client-view' }>;

const SESSION_ID = '1'.padStart(32, '0');
const CLIENT_ID = 'c'.padStart(32, '0');
const CONTEXT_ID = '2'.padStart(32, '0');
const TAB_ID = '3'.padStart(32, '0');
const [PANEL_A, PANEL_B, PANEL_C, PANEL_D, PANEL_E, PANEL_F] = ['4', '5', '6', '7', '8', '9'].map(
	(id) => id.padStart(32, '0'),
);
const OPERATION_ONE = 'a'.padStart(32, '0');
const OPERATION_TWO = 'b'.padStart(32, '0');
const LANE_ID = 7n;

interface Exchange {
	readonly frameKind: number;
	readonly reject: (error: Error) => void;
	readonly request: ClientViewRequest;
	readonly resolve: (record: Uint8Array) => void;
}

/** Holds each Synchronize until the test answers it, as the Server would. */
class FakeConnection {
	readonly closedLanes: bigint[] = [];
	readonly exchanges: Exchange[] = [];
	handlers: Handlers | null = null;

	openLane(): bigint {
		return LANE_ID;
	}

	closeLane(laneId: bigint): void {
		this.closedLanes.push(laneId);
	}

	subscribe(mode: string, _request: Uint8Array, handlers: Handlers, laneId?: bigint) {
		if (mode !== 'invalidations' || laneId !== LANE_ID) {
			throw new Error('Expected an invalidation stream on the view lane.');
		}
		this.handlers = handlers;
		return { close: () => undefined, resnapshot: () => undefined };
	}

	exchange(
		request: Uint8Array,
		options: { readonly laneId?: bigint; readonly timeoutMilliseconds?: number } = {},
	): Promise<Uint8Array> {
		// As in the TUI, a Synchronize goes on the view lane and waits two seconds at most.
		if (options.laneId !== LANE_ID || options.timeoutMilliseconds !== 2_000) {
			throw new Error('Expected a bounded exchange on the view lane.');
		}
		const parsed = parseWorkbenchRequest(
			JSON.parse(new TextDecoder().decode(request.subarray(12))),
		);
		if (parsed.kind !== 'workbench.client-view') throw new Error('Expected a Client View.');
		const frameKind = new DataView(request.buffer, request.byteOffset).getUint16(6);
		return new Promise((resolve, reject) => {
			this.exchanges.push({ frameKind, reject, request: parsed, resolve });
		});
	}
}

function shows(panelId: string | undefined): ServerClientViewSample {
	if (panelId === undefined) throw new Error('Expected a Panel.');
	return {
		phase: 'ready',
		selection: {
			panelCount: 4,
			panelId,
			tabId: TAB_ID,
			terminalContextId: CONTEXT_ID,
			zoomed: false,
		},
		workbenchRevision: '1',
	};
}

function synchronizeOf({ request: { action } }: Exchange) {
	if (action.kind !== 'synchronize') throw new Error('Expected a Synchronize.');
	return action;
}

/** What the Server read from each Synchronize: its sequence, the Panel shown, and any answer. */
function sent(connection: FakeConnection) {
	return connection.exchanges.map((exchange) => {
		const { completion, heartbeat, view, viewRevision } = synchronizeOf(exchange);
		return [heartbeat, viewRevision, view.selection?.panelId, completion];
	});
}

/** Answers a Synchronize, delivering the control named, which the Server admits against that view. */
function answer(
	connection: FakeConnection,
	index: number,
	operationId: string | null = null,
): void {
	const exchange = connection.exchanges[index];
	if (exchange === undefined) throw new Error(`Synchronize ${index + 1} never reached the Server.`);
	const { heartbeat, view, viewRevision } = synchronizeOf(exchange);
	const payload = new TextEncoder().encode(
		JSON.stringify({
			kind: 'workbench.client-view.result',
			outcome: {
				client: { availability: 'current', clientId: CLIENT_ID, heartbeat, view, viewRevision },
				control:
					operationId === null
						? null
						: {
								clientId: CLIENT_ID,
								expectedViewRevision: viewRevision,
								intent: {
									kind: 'panel',
									panelId: PANEL_B,
									tabId: TAB_ID,
									terminalContextId: CONTEXT_ID,
								},
								minimumWorkbenchRevision: '1',
								operationId,
							},
				kind: 'synchronized',
			},
			protocolVersion: 1,
			requestId: exchange.request.requestId,
			sessionId: SESSION_ID,
		}),
	);
	const record = new Uint8Array(12 + payload.byteLength);
	const header = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x57, 0x50]);
	header.setUint16(4, 1);
	header.setUint16(6, 26);
	header.setUint32(8, payload.byteLength);
	record.set(payload, 12);
	exchange.resolve(record);
}

function invalidate(connection: FakeConnection): void {
	if (connection.handlers === null) throw new Error('No invalidation stream opened.');
	connection.handlers.onFrame('update', new Uint8Array(8));
}

function settle(): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, 0));
}

function start(initial: ServerClientViewSample) {
	const connection = new FakeConnection();
	const controls: string[] = [];
	let current = initial;
	const clientView = ServerClientView.open({
		connection,
		onControl: ({ operationId }) => controls.push(operationId),
		sample: () => current,
		sessionId: SESSION_ID,
	});
	if (clientView === null) throw new Error('The view found no lane.');
	const show = (next: ServerClientViewSample): void => {
		current = next;
		clientView.changed();
	};
	return { clientView, connection, controls, show };
}

describe('ServerClientView', () => {
	it('advances the heartbeat with each Synchronize and the view revision only with the view', async () => {
		const { clientView, connection, show } = start(shows(PANEL_A));
		// A control queued before the stream opens would go unannounced, so nothing goes until then.
		clientView.changed();
		expect(connection.exchanges).toHaveLength(0);
		invalidate(connection);
		// A commit announced while one is in flight sends again once it is answered.
		invalidate(connection);
		answer(connection, 0);
		await settle();
		answer(connection, 1);
		await settle();
		show(shows(PANEL_B));
		answer(connection, 2);
		await settle();
		show(shows(PANEL_C));
		// A lost answer may have carried a control, so the view retires and nothing goes again.
		connection.exchanges[3]?.reject(new Error('The exchange timed out.'));
		await settle();
		invalidate(connection);
		show(shows(PANEL_D));

		expect(sent(connection)).toEqual([
			['1', '1', PANEL_A, null],
			['2', '1', PANEL_A, null],
			['3', '2', PANEL_B, null],
			['4', '3', PANEL_C, null],
		]);
		expect(connection.exchanges.map(({ frameKind }) => frameKind)).toEqual([25, 25, 25, 25]);
		expect(connection.closedLanes).toEqual([LANE_ID]);
	});

	it('runs a delivered control once and answers it in the view it was drawn in', async () => {
		const { clientView, connection, controls, show } = start(shows(PANEL_A));
		invalidate(connection);
		answer(connection, 0, OPERATION_ONE);
		await settle();
		show(shows(PANEL_B));
		clientView.complete(OPERATION_ONE, { kind: 'applied' }, shows(PANEL_B));
		clientView.complete(OPERATION_ONE, { kind: 'rejected', reason: 'busy' });
		// The window moves on before the answer leaves; the answer keeps the view it was drawn in.
		show(shows(PANEL_C));
		answer(connection, 1);
		await settle();
		answer(connection, 2);
		await settle();
		// The window moves again before the next control arrives, so that control never runs.
		show(shows(PANEL_D));
		answer(connection, 3, OPERATION_TWO);
		await settle();

		expect(controls).toEqual([OPERATION_ONE]);
		expect(sent(connection)).toEqual([
			['1', '1', PANEL_A, null],
			['2', '2', PANEL_B, null],
			['3', '2', PANEL_B, { operationId: OPERATION_ONE, outcome: { kind: 'applied' } }],
			['4', '3', PANEL_C, null],
			[
				'5',
				'4',
				PANEL_D,
				{ operationId: OPERATION_TWO, outcome: { kind: 'rejected', reason: 'stale-view' } },
			],
		]);
	});

	it('binds a view to the first record that reports it, and refuses one the window left', async () => {
		const { clientView, connection, show } = start(shows(PANEL_A));
		invalidate(connection);
		answer(connection, 0);
		await settle();
		// The Server already holds the view shown, so nothing more is sent.
		const acknowledged = await clientView.bind(shows(PANEL_A));
		show(shows(PANEL_B));
		show(shows(PANEL_C));
		// The view in flight is older than the one bound, so its answer decides nothing.
		const reported = clientView.bind(shows(PANEL_C));
		answer(connection, 1);
		await settle();
		answer(connection, 2);
		await settle();
		show(shows(PANEL_D));
		show(shows(PANEL_E));
		const left = clientView.bind(shows(PANEL_E));
		// The window moves on before reporting the view bound, as a person acting twice would.
		show(shows(PANEL_F));
		answer(connection, 3);
		await settle();
		answer(connection, 4);

		expect([acknowledged, await reported, await left]).toEqual([
			{ clientId: CLIENT_ID, kind: 'bound', viewRevision: '1' },
			{ clientId: CLIENT_ID, kind: 'bound', viewRevision: '3' },
			{ kind: 'stale' },
		]);
		expect(sent(connection).map(([, viewRevision, panelId]) => [viewRevision, panelId])).toEqual([
			['1', PANEL_A],
			['2', PANEL_B],
			['3', PANEL_C],
			['4', PANEL_D],
			['5', PANEL_F],
		]);
	});
});
