import type { MessageEvent, MessagePortMain } from 'electron';

import {
	TerminalDocumentControlEventSchema,
	TerminalDocumentControlCommandSchema,
	type TerminalDocumentControlCommand,
	type TerminalDocumentControlEvent,
	type TerminalDocumentControlOperationResult,
} from '../../shared';

export interface TerminalDocumentControlOperations {
	execute(command: TerminalDocumentControlCommand): Promise<TerminalDocumentControlOperationResult>;
}

export type TerminalDocumentControlResultPosted = (
	command: TerminalDocumentControlCommand,
	result: TerminalDocumentControlOperationResult,
) => void;

export class TerminalDocumentControlBroker {
	readonly #acceptedOperations = new Map<string, string>();
	readonly #operations: TerminalDocumentControlOperations;
	readonly #resultPosted: TerminalDocumentControlResultPosted | undefined;
	readonly #port: MessagePortMain;
	#disposed = false;
	#started = false;

	constructor(
		port: MessagePortMain,
		operations: TerminalDocumentControlOperations,
		resultPosted?: TerminalDocumentControlResultPosted,
	) {
		this.#port = port;
		this.#operations = operations;
		this.#resultPosted = resultPosted;
	}

	start(): void {
		if (this.#disposed || this.#started) return;
		this.#started = true;
		this.#port.on('message', this.#handleRendererMessage);
		this.#port.on('close', this.#handlePortClose);
		this.#port.start();
	}

	dispose(): void {
		this.#retire(true);
	}

	publishTerminalExit(
		event: Extract<TerminalDocumentControlEvent, { readonly type: 'terminal-exited' }>,
	): boolean {
		if (this.#disposed) return false;
		const parsed = TerminalDocumentControlEventSchema.safeParse(event);
		if (!parsed.success || parsed.data.type !== 'terminal-exited') {
			this.#retire(true);
			return false;
		}
		return this.#post(parsed.data);
	}

	readonly #handlePortClose = (): void => {
		this.#retire(false);
	};

	readonly #handleRendererMessage = (event: MessageEvent): void => {
		if (this.#disposed) return;
		const command = TerminalDocumentControlCommandSchema.safeParse(event.data);
		if (!command.success) {
			this.#retire(true);
			return;
		}
		const fingerprint = JSON.stringify(command.data);
		const existingFingerprint = this.#acceptedOperations.get(command.data.operationId);
		if (existingFingerprint !== undefined) {
			if (existingFingerprint !== fingerprint) this.#retire(true);
			return;
		}
		this.#acceptedOperations.set(command.data.operationId, fingerprint);
		void this.#operations.execute(command.data).then(
			(result) => this.#complete(command.data, result),
			() => this.#fail(command.data),
		);
	};

	#complete(
		command: TerminalDocumentControlCommand,
		result: TerminalDocumentControlOperationResult,
	): void {
		if (this.#disposed) return;
		const parsed = TerminalDocumentControlEventSchema.safeParse(result);
		if (!parsed.success || !operationResultMatches(command, parsed.data)) {
			this.#retire(true);
			return;
		}
		if (!this.#post(parsed.data) || this.#resultPosted === undefined) return;
		try {
			this.#resultPosted(command, parsed.data);
		} catch {
			this.#retire(true);
		}
	}

	#fail(command: TerminalDocumentControlCommand): void {
		if (this.#disposed) return;
		const identity = {
			operationId: command.operationId,
			presentationId: command.presentationId,
		};
		let result: TerminalDocumentControlOperationResult;
		switch (command.type) {
			case 'terminal-provision-requested':
				result = {
					...identity,
					failure: { code: 'document-control-unavailable', retryable: true },
					type: 'terminal-provision-failed',
				};
				break;
			case 'terminal-restore-requested':
				result = {
					...identity,
					failure: { code: 'document-control-unavailable', retryable: true },
					type: 'terminal-restore-failed',
				};
				break;
			case 'terminal-close-requested':
				result = {
					...identity,
					failure: { code: 'document-control-unavailable', retryable: true },
					type: 'terminal-close-failed',
				};
				break;
		}
		this.#post(result);
	}

	#post(event: TerminalDocumentControlEvent): boolean {
		try {
			this.#port.postMessage(event);
			return true;
		} catch {
			this.#retire(true);
			return false;
		}
	}

	#retire(closePort: boolean): void {
		if (this.#disposed) return;
		this.#disposed = true;
		if (this.#started) {
			this.#port.off('message', this.#handleRendererMessage);
			this.#port.off('close', this.#handlePortClose);
		}
		if (closePort) this.#port.close();
	}
}

function operationResultMatches(
	command: TerminalDocumentControlCommand,
	event: TerminalDocumentControlEvent,
): event is TerminalDocumentControlOperationResult {
	if (
		event.type === 'terminal-exited' ||
		event.operationId !== command.operationId ||
		event.presentationId !== command.presentationId
	) {
		return false;
	}
	switch (command.type) {
		case 'terminal-provision-requested':
			return (
				event.type === 'terminal-provision-completed' || event.type === 'terminal-provision-failed'
			);
		case 'terminal-restore-requested':
			return (
				event.type === 'terminal-restore-completed' || event.type === 'terminal-restore-failed'
			);
		case 'terminal-close-requested':
			return event.type === 'terminal-close-completed' || event.type === 'terminal-close-failed';
	}
}
