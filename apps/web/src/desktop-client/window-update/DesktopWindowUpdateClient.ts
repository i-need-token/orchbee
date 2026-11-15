import {
	DesktopWindowUpdateMainMessageSchema,
	type DesktopWindowUpdateRendererMessage,
} from '@orchbee/desktop-client-protocol';

import type { WorkbenchClientUpdate } from '../../workbench';

/**
 * This document's window-update port: the newer window version Main offers, or none, the request to
 * switch to it, and Main's answer when the switch has to wait until no terminal runs.
 */
export class DesktopWindowUpdateClient implements WorkbenchClientUpdate {
	#disposed = false;
	#offer: string | null = null;
	readonly #listeners = new Set<() => void>();
	readonly #port: MessagePort;
	readonly #waitingListeners = new Set<() => void>();

	constructor(port: MessagePort) {
		this.#port = port;
		port.addEventListener('message', ({ data }) => this.#receive(data));
		port.start();
	}

	readonly getSnapshot = (): string | null => this.#offer;

	readonly request = (version: string): void => {
		if (this.#disposed) return;
		const message: DesktopWindowUpdateRendererMessage = {
			type: 'desktop-window-update-requested',
			version,
		};
		this.#port.postMessage(message);
	};

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	readonly subscribeWaiting = (listener: () => void): (() => void) => {
		this.#waitingListeners.add(listener);
		return () => this.#waitingListeners.delete(listener);
	};

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#listeners.clear();
		this.#waitingListeners.clear();
		this.#port.close();
	}

	#receive(data: unknown): void {
		const message = DesktopWindowUpdateMainMessageSchema.safeParse(data);
		if (this.#disposed || !message.success) return;
		if (message.data.type === 'desktop-window-update-waiting') {
			for (const listener of this.#waitingListeners) listener();
			return;
		}
		if (message.data.version === this.#offer) return;
		this.#offer = message.data.version;
		for (const listener of this.#listeners) listener();
	}
}
