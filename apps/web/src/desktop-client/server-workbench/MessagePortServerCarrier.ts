import type { DesktopServerConnectionRead } from '@orchbee/desktop-client-protocol';

import type { ServerCarrier } from '../../workbench-client';

/**
 * Carries Client connection bytes over the port Desktop Main relays to `orchbee server connect`.
 * Each chunk is reported read once its handling returns, which is what lets Main relay more.
 */
export class MessagePortServerCarrier implements ServerCarrier {
	readonly #port: MessagePort;

	constructor(port: MessagePort) {
		this.#port = port;
	}

	close(): void {
		this.#port.close();
	}

	send(bytes: Uint8Array): void {
		this.#port.postMessage(bytes);
	}

	subscribe(receive: (chunk: Uint8Array) => void, closed: () => void): () => void {
		const handleMessage = (event: MessageEvent<unknown>): void => {
			const chunk = event.data;
			if (!(chunk instanceof Uint8Array) || chunk.byteLength === 0) return;
			try {
				receive(chunk);
			} finally {
				// A chunk whose handling failed is still read; withholding it would stall the connection.
				const read: DesktopServerConnectionRead = {
					byteLength: chunk.byteLength,
					type: 'desktop-server-connection-read',
				};
				this.#port.postMessage(read);
			}
		};
		const handleClose = (): void => closed();
		this.#port.addEventListener('message', handleMessage);
		this.#port.addEventListener('close', handleClose);
		this.#port.start();
		return () => {
			this.#port.removeEventListener('message', handleMessage);
			this.#port.removeEventListener('close', handleClose);
		};
	}
}
