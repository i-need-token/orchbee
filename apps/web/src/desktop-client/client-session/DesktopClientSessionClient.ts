import type {
	WorkbenchClientSessionBootstrap,
	WorkbenchClientSessionRequestId,
} from '@orchbee/workbench-client-protocol';

import {
	WorkbenchClientSessionClient,
	type WorkbenchClientSessionTransport,
} from '../../workbench-client';

export class DesktopClientSessionClient extends WorkbenchClientSessionClient {
	constructor(
		port: MessagePort,
		bootstrap: WorkbenchClientSessionBootstrap,
		createRequestId?: () => WorkbenchClientSessionRequestId,
	) {
		super(new MessagePortWorkbenchClientSessionTransport(port), bootstrap, createRequestId);
	}
}

class MessagePortWorkbenchClientSessionTransport implements WorkbenchClientSessionTransport {
	readonly #port: MessagePort;

	constructor(port: MessagePort) {
		this.#port = port;
	}

	close(): void {
		this.#port.close();
	}

	send(command: Parameters<WorkbenchClientSessionTransport['send']>[0]): void {
		this.#port.postMessage(command);
	}

	subscribe(listener: (value: unknown) => void, unavailable: () => void): () => void {
		const handleMessage = (message: MessageEvent<unknown>): void => {
			if (message.ports.length !== 0) {
				for (const transferredPort of message.ports) transferredPort.close();
				unavailable();
				return;
			}
			listener(message.data);
		};
		const handleMessageError = (): void => unavailable();
		this.#port.addEventListener('message', handleMessage);
		this.#port.addEventListener('messageerror', handleMessageError);
		this.#port.start();
		return () => {
			this.#port.removeEventListener('message', handleMessage);
			this.#port.removeEventListener('messageerror', handleMessageError);
		};
	}
}
