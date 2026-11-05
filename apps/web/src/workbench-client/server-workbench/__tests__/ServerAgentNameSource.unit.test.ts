import { parseWorkbenchRequest, parseWorkbenchResponse } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchAgentObservation } from '../../../workbench';
import { ServerAgentNameSource } from '../ServerAgentNameSource';
import type { ServerTerminalAgentBinding } from '../ServerTerminalAgentSource';

const hex = (value: number): string => value.toString(16).padStart(32, '0');
const PRESENTATION = hex(0x10);
const WORKER = hex(0x20);
const TERMINAL = hex(0x21);
const BINDING = {
	generation: hex(0x30),
	hostEpoch: hex(0x31),
	hostTerminalId: hex(0x32),
	presentationId: PRESENTATION,
};

/** The session and binding as the Server's own answer types them. */
function stateFixture(): {
	readonly binding: ServerTerminalAgentBinding;
	readonly sessionId: ReturnType<typeof parseStates>['sessionId'];
} {
	const response = parseStates();
	const binding =
		response.outcome.kind === 'states'
			? response.outcome.presentations[0]?.snapshot?.binding
			: null;
	if (binding === null || binding === undefined) throw new Error('The fixture has no binding.');
	return { binding, sessionId: response.sessionId };
}

function parseStates() {
	const response = parseWorkbenchResponse({
		kind: 'workbench.panel-agent.result',
		outcome: {
			kind: 'states',
			presentations: [
				{
					presentationId: PRESENTATION,
					snapshot: { agent: 'claude', binding: BINDING, sequence: '1', state: 'working' },
				},
			],
			reportRevision: '1',
			workbenchRevision: '1',
		},
		protocolVersion: 1,
		requestId: hex(0xf0),
		sessionId: hex(0x1),
	});
	if (response.kind !== 'workbench.panel-agent.result') throw new Error('Not a state answer.');
	return response;
}

function record(magic: readonly number[], frameKind: number, payload: unknown): Uint8Array {
	const body = new TextEncoder().encode(JSON.stringify(payload));
	const frame = new Uint8Array(12 + body.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(magic);
	view.setUint16(4, 1);
	view.setUint16(6, frameKind);
	view.setUint32(8, body.byteLength);
	frame.set(body, 12);
	return frame;
}

const metadataRecord = (source: string, agent: string | null, applies: string | null) => ({
	agent,
	appliesToSource: applies,
	displayAgent: null,
	source,
	stateLabels: {},
});

/** Answers each read as a Server holding one named, titled and reported Claude terminal would. */
function exchange(request: Uint8Array): Promise<Uint8Array> {
	const payload: unknown = JSON.parse(new TextDecoder().decode(request.subarray(12)));
	if (request[2] === 0x41) {
		// OBAP: the names list. A name bound to another incarnation of the terminal names nothing.
		const requestId =
			typeof payload === 'object' && payload !== null && 'requestId' in payload
				? payload.requestId
				: null;
		return Promise.resolve(
			record([0x4f, 0x42, 0x41, 0x50], 10, {
				kind: 'agent.session.names.list.result',
				requestId,
				result: {
					outcome: {
						bindings: [
							{ name: 'scout', target: { binding: BINDING, kind: 'terminal' } },
							{
								name: 'stale',
								target: { binding: { ...BINDING, generation: hex(0x99) }, kind: 'terminal' },
							},
						],
						kind: 'listed-targets',
						pending: [],
					},
				},
			}),
		);
	}
	const parsed = parseWorkbenchRequest(payload);
	if (parsed.kind !== 'workbench.panel-metadata' && parsed.kind !== 'workbench.panel-agent') {
		throw new Error(`Unexpected ${parsed.kind}.`);
	}
	const envelope = {
		protocolVersion: 1,
		requestId: parsed.requestId,
		sessionId: parsed.sessionId,
	};
	const panel = { binding: { terminalId: TERMINAL, workerId: WORKER }, panelId: hex(0x40) };
	if (parsed.kind === 'workbench.panel-metadata') {
		return Promise.resolve(
			record([0x4f, 0x42, 0x57, 0x50], 34, {
				...envelope,
				kind: 'workbench.panel-metadata.result',
				outcome: {
					kind: 'presentations-read',
					metadataRevision: '9',
					presentations: [
						{
							panel: {
								...panel,
								presentations: [
									// Written last, but for another Agent.
									{ ...metadataRecord('a', 'codex', null), title: { order: '9', value: 'Other' } },
									// Guarded by the report source this Agent reports from.
									{
										...metadataRecord('b', 'claude', 'hook'),
										title: { order: '5', value: 'Ship login' },
									},
									{ ...metadataRecord('c', null, null), title: { order: '3', value: 'Older' } },
								],
								tokens: {},
							},
							presentationId: PRESENTATION,
						},
					],
					workbenchRevision: '1',
				},
			}),
		);
	}
	const outcome =
		parsed.action.kind === 'titles'
			? {
					kind: 'titles',
					presentations: [
						{
							binding: BINDING,
							observation: { kind: 'observed', revision: '2', title: '✳ Fix tests' },
						},
					],
					workbenchRevision: '1',
				}
			: {
					kind: 'presentations-read',
					presentations: [
						{
							panel: {
								...panel,
								report: {
									agent: 'claude',
									authoritySource: 'hook',
									message: null,
									state: 'working',
								},
							},
							presentationId: PRESENTATION,
						},
					],
					reportRevision: '1',
					workbenchRevision: '1',
				};
	return Promise.resolve(
		record([0x4f, 0x42, 0x57, 0x50], 36, {
			...envelope,
			kind: 'workbench.panel-agent.result',
			outcome,
		}),
	);
}

describe('ServerAgentNameSource', () => {
	it('names an Agent by its exact incarnation, its guarded metadata title, and its read terminal title', async () => {
		const { binding, sessionId } = stateFixture();
		const observation: WorkbenchAgentObservation = {
			agent: 'claude',
			binding: 'incarnation',
			sequence: 1n,
			state: 'working',
		};
		const source = new ServerAgentNameSource(
			{ exchange },
			{ onEnded: () => () => undefined, snapshot: { sessionId } },
			{
				getSnapshot: () => ({
					complete: true,
					presentations: new Map([[PRESENTATION, observation]]),
				}),
				subscribe: () => () => undefined,
				terminalBinding: () => binding,
			},
		);
		await vi.waitFor(() => {
			expect(source.getSnapshot().get(PRESENTATION)).toEqual({
				binding: 'incarnation',
				name: 'scout',
				reportedTitle: 'Ship login',
				terminalTitle: 'Fix tests',
			});
		});
		source.dispose();
		expect(source.getSnapshot().size).toBe(0);
	});
});
