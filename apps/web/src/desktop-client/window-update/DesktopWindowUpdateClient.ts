import {
	DesktopWindowUpdateMainMessageSchema,
	type DesktopRelaunchPlace,
	type DesktopWindowUpdateRendererMessage,
} from '@orchbee/desktop-client-protocol';

import type { WorkbenchClientUpdate } from '../../workbench';

/**
 * This document's window-update port: the newer window version Main offers, or none, the request to
 * switch to it with the place the window shows, Main's answer when the switch has to wait until no
 * terminal runs, and the place it answers when Main asks for one.
 */
export class DesktopWindowUpdateClient implements WorkbenchClientUpdate {
	#disposed = false;
	#offer: string | null = null;
	readonly #failedListeners = new Set<() => void>();
	readonly #hostReplacementListeners = new Set<(terminals: number | null) => void>();
	readonly #listeners = new Set<() => void>();
	readonly #port: MessagePort;
	readonly #readPlace: () => DesktopRelaunchPlace | null;
	readonly #waitingListeners = new Set<() => void>();

	/** `readPlace` names where the relaunched window opens, read when a switch or a place request asks. */
	constructor(port: MessagePort, readPlace: () => DesktopRelaunchPlace | null) {
		this.#port = port;
		this.#readPlace = readPlace;
		port.addEventListener('message', ({ data }) => this.#receive(data));
		port.start();
	}

	readonly getSnapshot = (): string | null => this.#offer;

	/** Asks Main to re-read `active.json` from disk once an update check settles. */
	readonly refresh = (): void => {
		if (this.#disposed) return;
		const message: DesktopWindowUpdateRendererMessage = {
			type: 'desktop-window-update-refresh',
		};
		this.#port.postMessage(message);
	};

	readonly request = (version: string): void => {
		if (this.#disposed) return;
		const message: DesktopWindowUpdateRendererMessage = {
			place: this.#readPlace(),
			type: 'desktop-window-update-requested',
			version,
		};
		this.#port.postMessage(message);
	};

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	/** Answers the Host replacement question Main asked: whether every terminal may restart. */
	readonly answerHostReplacement = (confirmed: boolean): void => {
		if (this.#disposed) return;
		const message: DesktopWindowUpdateRendererMessage = {
			confirmed,
			type: 'desktop-window-update-host-replacement-answer',
		};
		this.#port.postMessage(message);
	};

	readonly subscribeHostReplacement = (
		listener: (terminals: number | null) => void,
	): (() => void) => {
		this.#hostReplacementListeners.add(listener);
		return () => this.#hostReplacementListeners.delete(listener);
	};

	readonly subscribeFailed = (listener: () => void): (() => void) => {
		this.#failedListeners.add(listener);
		return () => this.#failedListeners.delete(listener);
	};

	readonly subscribeWaiting = (listener: () => void): (() => void) => {
		this.#waitingListeners.add(listener);
		return () => this.#waitingListeners.delete(listener);
	};

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#failedListeners.clear();
		this.#hostReplacementListeners.clear();
		this.#listeners.clear();
		this.#waitingListeners.clear();
		this.#port.close();
	}

	/** Answers Main's place request with the place the window shows, naming the request it answers. */
	#answerPlace(requestId: string): void {
		const message: DesktopWindowUpdateRendererMessage = {
			place: this.#readPlace(),
			requestId,
			type: 'desktop-window-update-place',
		};
		this.#port.postMessage(message);
	}

	#receive(data: unknown): void {
		const message = DesktopWindowUpdateMainMessageSchema.safeParse(data);
		if (this.#disposed || !message.success) return;
		if (message.data.type === 'desktop-window-update-place-requested') {
			this.#answerPlace(message.data.requestId);
			return;
		}
		if (message.data.type === 'desktop-window-update-failed') {
			for (const listener of this.#failedListeners) listener();
			return;
		}
		if (message.data.type === 'desktop-window-update-host-replacement') {
			for (const listener of this.#hostReplacementListeners) listener(message.data.terminals);
			return;
		}
		if (message.data.type === 'desktop-window-update-waiting') {
			for (const listener of this.#waitingListeners) listener();
			return;
		}
		if (message.data.version === this.#offer) return;
		this.#offer = message.data.version;
		for (const listener of this.#listeners) listener();
	}
}
