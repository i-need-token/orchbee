import {
	DesktopApplicationLifetimeMainEventSchema,
	DesktopApplicationLifetimeRendererCommandSchema,
	type DesktopApplicationLifetimeMainEvent,
	type DesktopApplicationLifetimeRendererCommand,
} from '@orchbee/desktop-client-protocol';

type Listener = (event: DesktopApplicationLifetimeMainEvent) => void;

export class DesktopApplicationLifetimeClient {
	#disposed = false;
	readonly #listeners = new Set<Listener>();
	readonly #port: MessagePort;

	constructor(port: MessagePort) {
		this.#port = port;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	subscribe(listener: Listener): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	post(command: DesktopApplicationLifetimeRendererCommand): boolean {
		if (this.#disposed) return false;
		const parsed = DesktopApplicationLifetimeRendererCommandSchema.safeParse(command);
		if (!parsed.success) {
			this.dispose();
			return false;
		}
		try {
			this.#port.postMessage(parsed.data);
			return true;
		} catch {
			this.dispose();
			return false;
		}
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#listeners.clear();
		this.#port.close();
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.dispose();
			return;
		}
		const parsed = DesktopApplicationLifetimeMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.dispose();
			return;
		}
		for (const listener of this.#listeners) listener(parsed.data);
	};

	readonly #handleMessageError = (): void => {
		this.dispose();
	};
}
