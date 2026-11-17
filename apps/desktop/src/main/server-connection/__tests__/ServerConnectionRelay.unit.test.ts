import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';

import { describe, expect, it } from 'vitest';

import type { DesktopDiagnosticRecorder } from '../../diagnostics';
import {
	ServerConnectionRelay,
	type ServerConnectProcess,
	type ServerConnectionPort,
} from '../ServerConnectionRelay';

const CHUNK_BYTES = 64 * 1024;
/** More than any bound a relay may keep in Main for one connection. */
const FLOOD_BYTES = 256 * 1024 * 1024;

/** A recorder that keeps what it was given. */
function createRecorder(): {
	readonly records: unknown[][];
	readonly recordEvent: DesktopDiagnosticRecorder;
} {
	const records: unknown[][] = [];
	return { recordEvent: (...record) => void records.push(record), records };
}

class FakePort extends EventEmitter implements ServerConnectionPort {
	closed = false;
	readonly posted: unknown[] = [];

	close(): void {
		this.closed = true;
	}

	postMessage(message: unknown): void {
		this.posted.push(message);
	}

	start(): void {}

	fromPage(data: unknown): void {
		this.emit('message', { data });
	}

	unreadBytes(): number {
		return this.posted.reduce<number>(
			(total, message) => total + (message instanceof Uint8Array ? message.byteLength : 0),
			0,
		);
	}
}

/** A process whose standard input never takes anything, as one that stopped reading. */
function createProcess(): {
	process: ServerConnectProcess;
	stdin: Writable;
	stdout: PassThrough;
} {
	const stdout = new PassThrough({ highWaterMark: CHUNK_BYTES });
	const stdin = new Writable({ write: () => undefined });
	const process = Object.assign(new EventEmitter(), { stdin, stdout });
	return { process: process as unknown as ServerConnectProcess, stdin, stdout };
}

/** Writes output, a chunk per turn as a pipe delivers it, until the relay stops taking it. */
async function flood(stdout: PassThrough): Promise<void> {
	const chunk = Buffer.alloc(CHUNK_BYTES, 1);
	for (let written = 0; written < FLOOD_BYTES && !stdout.isPaused(); written += CHUNK_BYTES) {
		stdout.write(chunk);
		await new Promise((resolve) => setImmediate(resolve));
	}
}

describe('ServerConnectionRelay', () => {
	it('stops relaying output a page has not read, and relays it again once the page reads', async () => {
		const { process, stdout } = createProcess();
		const port = new FakePort();
		new ServerConnectionRelay(process, port, createRecorder().recordEvent);

		await flood(stdout);
		expect(stdout.isPaused()).toBe(true);
		const unread = port.unreadBytes();
		expect(unread).toBeLessThan(FLOOD_BYTES);

		port.fromPage({ byteLength: unread, type: 'desktop-server-connection-read' });
		expect(stdout.isPaused()).toBe(false);
		expect(port.closed).toBe(false);
	});

	it('keeps output stopped for a report of more than was relayed, and records it', async () => {
		const { process, stdout } = createProcess();
		const port = new FakePort();
		const { recordEvent, records } = createRecorder();
		new ServerConnectionRelay(process, port, recordEvent);

		await flood(stdout);
		port.fromPage({ byteLength: port.unreadBytes() + 1, type: 'desktop-server-connection-read' });
		port.fromPage({ type: 'something-else' });

		expect(stdout.isPaused()).toBe(true);
		expect(port.closed).toBe(false);
		expect(records).toContainEqual([
			'warning',
			'server-relay-message-dropped',
			{ reason: 'over-read' },
		]);
	});

	it('ends only its own connection when the process stops taking the page input', () => {
		const { process, stdin } = createProcess();
		const port = new FakePort();
		const { recordEvent, records } = createRecorder();
		new ServerConnectionRelay(process, port, recordEvent);

		const bytes = new Uint8Array(CHUNK_BYTES * 16);
		for (let sent = 0; sent < FLOOD_BYTES && !port.closed; sent += bytes.byteLength) {
			port.fromPage(bytes);
		}

		expect(port.closed).toBe(true);
		expect(stdin.writableEnded).toBe(true);
		expect(records).toContainEqual(['warning', 'server-relay-ended', { reason: 'input-bound' }]);
	});
});
