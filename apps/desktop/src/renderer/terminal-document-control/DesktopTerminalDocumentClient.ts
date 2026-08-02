import type {
	TerminalPresentationCommand,
	TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';

import {
	TerminalDocumentControlCommandSchema,
	TerminalDocumentControlEventSchema,
	type TerminalDocumentControlEvent,
} from '../../shared';

type TerminalDocumentEventListener = (event: TerminalPresentationEvent) => void;

export class DesktopTerminalDocumentClient {
	readonly #listeners = new Set<TerminalDocumentEventListener>();
	readonly #port: MessagePort;
	#disposed = false;

	constructor(port: MessagePort) {
		this.#port = port;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleMessageError);
		port.start();
	}

	dispatch(commands: readonly TerminalPresentationCommand[]): void {
		for (const command of commands) {
			if (this.#disposed) {
				this.#publishLocalFailure(command);
				continue;
			}
			const parsed = TerminalDocumentControlCommandSchema.safeParse(command);
			if (!parsed.success) {
				this.dispose();
				this.#publishLocalFailure(command);
				continue;
			}
			try {
				this.#port.postMessage(parsed.data);
			} catch {
				this.dispose();
				this.#publishLocalFailure(command);
			}
		}
	}

	subscribe(listener: TerminalDocumentEventListener): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleMessageError);
		this.#port.close();
		this.#listeners.clear();
	}

	readonly #handleMessage = (message: MessageEvent<unknown>): void => {
		if (this.#disposed || message.ports.length !== 0) {
			this.dispose();
			return;
		}
		const event = TerminalDocumentControlEventSchema.safeParse(message.data);
		if (!event.success) {
			this.dispose();
			return;
		}
		this.#publish(event.data);
	};

	readonly #handleMessageError = (): void => {
		this.dispose();
	};

	#publish(event: TerminalDocumentControlEvent): void {
		for (const listener of this.#listeners) listener(event);
	}

	#publishLocalFailure(command: TerminalPresentationCommand): void {
		this.#publish(
			command.type === 'terminal-provision-requested'
				? {
						failure: { code: 'document-control-unavailable', retryable: true },
						operationId: command.operationId,
						presentationId: command.presentationId,
						type: 'terminal-provision-failed',
					}
				: {
						failure: { code: 'document-control-unavailable', retryable: true },
						operationId: command.operationId,
						presentationId: command.presentationId,
						type: 'terminal-close-failed',
					},
		);
	}
}
