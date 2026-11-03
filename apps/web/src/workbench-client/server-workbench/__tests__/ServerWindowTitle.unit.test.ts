import { parseWorkbenchRequest } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import type { ServerConnection } from '../../server-connection';
import { ServerWindowTitle } from '../ServerWindowTitle';

type Handlers = Parameters<ServerConnection['subscribe']>[2];

const SESSION_ID = '1'.padStart(32, '0');

interface Context {
	readonly pane: string;
	readonly tab: string;
	readonly terminalTitle: string;
	readonly workspace: string;
}

interface FakeStream {
	readonly context: Context;
	readonly focused: boolean;
	readonly handlers: Handlers;
	readonly requestId: string;
}

/** Records the order in which streams open and close, as the Server would see them. */
class FakeConnection {
	readonly log: string[] = [];
	readonly streams: FakeStream[] = [];

	subscribe(mode: string, request: Uint8Array, handlers: Handlers) {
		const parsed = parseWorkbenchRequest(
			JSON.parse(new TextDecoder().decode(request.subarray(12))),
		);
		if (
			mode !== 'state' ||
			parsed.kind !== 'workbench.window-title' ||
			parsed.action.kind !== 'listen'
		) {
			throw new Error('Expected a state stream that listens for the window title.');
		}
		const number = this.streams.length + 1;
		this.streams.push({
			context: parsed.action.context,
			focused: parsed.action.focused,
			handlers,
			requestId: parsed.requestId,
		});
		this.log.push(`open ${number}`);
		return {
			close: () => this.log.push(`close ${number}`),
			resnapshot: () => {
				throw new Error('Every state frame is complete, so none is asked for again.');
			},
		};
	}
}

function context(workspace: string): Context {
	return { pane: '', tab: '', terminalTitle: '', workspace };
}

function listening(stream: FakeStream | undefined, snapshot: unknown): void {
	if (stream === undefined) throw new Error('No stream reached the Server.');
	const payload = new TextEncoder().encode(
		JSON.stringify({
			kind: 'workbench.window-title.result',
			outcome: { kind: 'listening', snapshot },
			protocolVersion: 1,
			requestId: stream.requestId,
			sessionId: SESSION_ID,
		}),
	);
	const record = new Uint8Array(12 + payload.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x57, 0x50]);
	view.setUint16(4, 1);
	view.setUint16(6, 44);
	view.setUint32(8, payload.byteLength);
	record.set(payload, 12);
	stream.handlers.onFrame('update', record);
}

function start() {
	const connection = new FakeConnection();
	const shown: string[] = [];
	const title = new ServerWindowTitle(connection, SESSION_ID, (text) => shown.push(text));
	return { connection, shown, title };
}

describe('ServerWindowTitle', () => {
	it('listens anew only for a new focus or context, opening before it closes', () => {
		const { connection, title } = start();
		title.update(true, context('one'));
		title.update(true, context('one'));
		title.update(true, context('two'));
		title.update(false, context('two'));
		title.dispose();

		// Closing first would make the Server forget this window and bring it forward as a new one.
		expect(connection.log).toEqual(['open 1', 'open 2', 'close 1', 'open 3', 'close 2', 'close 3']);
		expect(
			connection.streams.map(({ context: { workspace }, focused }) => [workspace, focused]),
		).toEqual([
			['one', true],
			['two', true],
			['two', false],
		]);
	});

	it('shows each new title from the foreground and keeps it behind another Client', () => {
		const { connection, shown, title } = start();
		title.update(true, context('one'));
		listening(connection.streams[0], { revision: '1', title: 'host: one' });
		listening(connection.streams[0], { revision: '1', title: 'host: one' });
		// Behind another Client nothing arrives, and the title shown stays.
		listening(connection.streams[0], null);
		title.update(true, context('two'));
		// The replaced stream no longer names the window.
		listening(connection.streams[0], { revision: '9', title: 'stale' });
		listening(connection.streams[1], { revision: '2', title: null });
		// Focus returning brings the window forward, so the same title shows again.
		title.update(false, context('two'));
		title.update(true, context('two'));
		listening(connection.streams[3], { revision: '2', title: null });

		expect(shown).toEqual(['host: one', 'Orchbee', 'Orchbee']);
	});

	it('bounds each context value as the TUI does', () => {
		const { connection, title } = start();
		title.update(true, {
			pane: `left\u0007pane\ud800`,
			tab: '😀'.repeat(201),
			terminalTitle: 'build\nlog',
			workspace: 'one',
		});

		expect(connection.streams[0]?.context).toEqual({
			pane: 'leftpane',
			tab: '😀'.repeat(200),
			terminalTitle: 'buildlog',
			workspace: 'one',
		});
	});
});
