import type { ChildProcessByStdio } from 'node:child_process';
import type { Readable, Writable } from 'node:stream';

import { DesktopServerConnectionReadSchema } from '@orchbee/desktop-client-protocol';

import { containListenerFault, type DesktopDiagnosticRecorder } from '../diagnostics';

export type ServerConnectProcess = ChildProcessByStdio<Writable, Readable, null>;

export interface ServerConnectionPort {
	close(): void;
	on(event: 'close', listener: () => void): unknown;
	on(event: 'message', listener: (message: { readonly data: unknown }) => void): unknown;
	postMessage(message: unknown): void;
	start(): void;
}

/**
 * Output stops while the page has not read this much of what Main relayed, and starts again once
 * it has read down to half of it. A record larger than this still flows, because the page reports
 * each chunk read as it decodes it, not each record.
 */
const UNREAD_OUTPUT_LIMIT_BYTES = 4 * 1024 * 1024;
const UNREAD_OUTPUT_RESUME_BYTES = UNREAD_OUTPUT_LIMIT_BYTES / 2;
/**
 * The page's bytes the process has not taken yet. Two of the largest records the Client contract
 * admits (24 MiB each) fit, so only a process that stopped reading reaches it.
 */
const PENDING_INPUT_LIMIT_BYTES = 64 * 1024 * 1024;

/**
 * Moves one Client connection's bytes between a page's port and an `orchbee server connect` process.
 * Main reads nothing here: the page owns the protocol and the process owns activation, peer
 * validation, and admission. Both directions are bounded. Output pauses while the page is behind,
 * which leaves the bytes in the process and, past it, with the Server, whose own bound on a slow
 * Client then decides; a process that stops taking input past its bound ends this connection alone.
 */
export class ServerConnectionRelay {
	readonly #port: ServerConnectionPort;
	readonly #process: ServerConnectProcess;
	readonly #recordEvent: DesktopDiagnosticRecorder;
	#closed = false;
	#outputPaused = false;
	#refusalRecorded = false;
	#unreadOutput = 0;

	constructor(
		process: ServerConnectProcess,
		port: ServerConnectionPort,
		recordEvent: DesktopDiagnosticRecorder,
	) {
		this.#port = port;
		this.#process = process;
		this.#recordEvent = recordEvent;
		port.on(
			'message',
			containListenerFault(
				recordEvent,
				'port-server-connection',
				({ data }: { readonly data: unknown }) => {
					if (this.#closed) return;
					if (data instanceof Uint8Array) this.#relayInput(data);
					else this.#read(data);
				},
				undefined,
			),
		);
		port.on('close', () => this.close());
		process.stdout.on('data', (chunk: Buffer) => this.#relayOutput(chunk));
		process.stdout.once('close', () => this.close());
		process.stdout.on('error', () => this.close());
		process.stdin.on('error', () => this.close());
		process.once('error', () => this.close());
		port.start();
	}

	/**
	 * Closing standard input is how the process ends: it shuts the connection down and exits. Its
	 * output is let go too, so a process blocked writing what the page never read can end as well.
	 */
	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#process.stdin.end();
		this.#process.stdout.destroy();
		this.#port.close();
	}

	#relayInput(bytes: Uint8Array): void {
		const stdin = this.#process.stdin;
		stdin.write(bytes);
		if (stdin.writableLength <= PENDING_INPUT_LIMIT_BYTES) return;
		this.#recordEvent('warning', 'server-relay-ended', { reason: 'input-bound' });
		this.close();
	}

	#relayOutput(chunk: Buffer): void {
		if (this.#closed) return;
		// A pooled chunk's view would clone the whole backing buffer through the port.
		this.#port.postMessage(new Uint8Array(chunk));
		this.#unreadOutput += chunk.byteLength;
		if (this.#outputPaused || this.#unreadOutput < UNREAD_OUTPUT_LIMIT_BYTES) return;
		this.#outputPaused = true;
		this.#process.stdout.pause();
	}

	/**
	 * The page reports what it has read. A message that is not such a report, or reports more than
	 * was relayed, is dropped; the first of each connection is recorded.
	 */
	#read(data: unknown): void {
		const read = DesktopServerConnectionReadSchema.safeParse(data);
		if (!read.success || read.data.byteLength > this.#unreadOutput) {
			if (!this.#refusalRecorded) {
				this.#refusalRecorded = true;
				this.#recordEvent('warning', 'server-relay-message-dropped', {
					reason: read.success ? 'over-read' : 'unparsable',
				});
			}
			return;
		}
		this.#unreadOutput -= read.data.byteLength;
		if (!this.#outputPaused || this.#unreadOutput > UNREAD_OUTPUT_RESUME_BYTES) return;
		this.#outputPaused = false;
		this.#process.stdout.resume();
	}
}
