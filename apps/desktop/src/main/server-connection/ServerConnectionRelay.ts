import type { ChildProcessByStdio } from 'node:child_process';
import type { Readable, Writable } from 'node:stream';

export type ServerConnectProcess = ChildProcessByStdio<Writable, Readable, null>;

export interface ServerConnectionPort {
	close(): void;
	on(event: 'close', listener: () => void): unknown;
	on(event: 'message', listener: (message: { readonly data: unknown }) => void): unknown;
	postMessage(message: unknown): void;
	start(): void;
}

/**
 * Moves one Client connection's bytes between a page's port and an `orchbee server connect` process.
 * Main reads nothing here: the page owns the protocol and the process owns activation, peer
 * validation, and admission. Output needs no flow control of its own because the page pulls.
 */
export class ServerConnectionRelay {
	readonly #port: ServerConnectionPort;
	readonly #process: ServerConnectProcess;
	#closed = false;

	constructor(process: ServerConnectProcess, port: ServerConnectionPort) {
		this.#port = port;
		this.#process = process;
		port.on('message', ({ data }) => {
			if (data instanceof Uint8Array && !this.#closed) process.stdin.write(data);
		});
		port.on('close', () => this.close());
		process.stdout.on('data', (chunk: Buffer) => {
			// A pooled chunk's view would clone the whole backing buffer through the port.
			if (!this.#closed) port.postMessage(new Uint8Array(chunk));
		});
		process.stdout.once('close', () => this.close());
		process.stdin.on('error', () => this.close());
		process.once('error', () => this.close());
		port.start();
	}

	/** Closing standard input is how the process ends: it shuts the connection down and exits. */
	close(): void {
		if (this.#closed) return;
		this.#closed = true;
		this.#process.stdin.end();
		this.#port.close();
	}
}
