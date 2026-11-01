import type { ServerCarrier } from '../../workbench-client';

/** Carries Client connection bytes over the port Desktop Main relays to `orchbee server connect`. */
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
			if (event.data instanceof Uint8Array) receive(event.data);
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
