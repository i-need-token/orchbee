import type { MessageEvent, MessagePortMain } from 'electron';

import {
	DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
	DesktopClientSessionBootstrapSchema,
	DesktopClientSessionCommandSchema,
	DesktopClientSessionEventSchema,
	type DesktopClientSessionBootstrap,
} from '../../shared';
import type {
	DesktopClientSessionCoordinator,
	DesktopClientSessionHandle,
	DesktopClientSessionPublication,
	DesktopClientSessionPublisher,
} from './DesktopClientSessionCoordinator';

export interface DesktopClientSessionBrokerDependencies {
	readonly disposeAttachments: () => Promise<void>;
}

export class DesktopClientSessionBroker implements DesktopClientSessionPublisher {
	readonly #disposeAttachmentsCapability: () => Promise<void>;
	#disposed = false;
	readonly #handle: DesktopClientSessionHandle;
	readonly #port: MessagePortMain;
	readonly #requests = new Map<string, string>();

	constructor(
		port: MessagePortMain,
		coordinator: DesktopClientSessionCoordinator,
		dependencies: DesktopClientSessionBrokerDependencies,
	) {
		this.#port = port;
		this.#disposeAttachmentsCapability = dependencies.disposeAttachments;
		const registration = coordinator.registerSession(this);
		if (registration.kind !== 'ready') {
			port.close();
			throw new Error(`Desktop client session is unavailable: ${registration.reason}.`);
		}
		this.#handle = registration.handle;
	}

	get bootstrap(): DesktopClientSessionBootstrap {
		const documentState =
			this.#handle.bootstrap.kind === 'available'
				? { document: this.#handle.bootstrap.document, kind: 'available' as const }
				: { kind: 'missing' as const };
		return DesktopClientSessionBootstrapSchema.parse({
			clientSessionId: this.#handle.clientSessionId,
			defaultCwd: this.#handle.bootstrap.defaultCwd,
			documentState,
			initialPresentationId: this.#handle.bootstrap.initialPresentationId,
			initializationDocument: this.#handle.bootstrap.initializationDocument,
			sequence: this.#handle.bootstrap.sequence,
			terminalEvents: this.#handle.bootstrap.terminalEvents,
			type: DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
		});
	}

	start(): void {
		if (this.#disposed) return;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
	}

	dispose(): Promise<void> {
		return this.#handle.dispose();
	}

	publish(publication: DesktopClientSessionPublication): void {
		if (this.#disposed) throw new Error('Desktop client session port is closed.');
		const event = DesktopClientSessionEventSchema.parse(publication);
		this.#port.postMessage(event);
	}

	async disposeAttachments(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.off('message', this.#handleMessage);
		this.#port.off('close', this.#handleClose);
		this.#port.close();
		await this.#disposeAttachmentsCapability();
	}

	readonly #handleMessage = (message: MessageEvent): void => {
		if (this.#disposed) return;
		if (message.ports.length !== 0) {
			for (const port of message.ports) port.close();
			void this.dispose();
			return;
		}
		const command = DesktopClientSessionCommandSchema.safeParse(message.data);
		if (!command.success) {
			void this.dispose();
			return;
		}
		const fingerprint = JSON.stringify(command.data);
		const existing = this.#requests.get(command.data.requestId);
		if (existing === fingerprint) return;
		this.#requests.set(command.data.requestId, fingerprint);
		void this.#handle
			.propose(command.data.requestId, command.data.expectedRevision, command.data.document)
			.then((result) => {
				if (this.#disposed) return;
				try {
					this.#port.postMessage(
						DesktopClientSessionEventSchema.parse({
							requestId: command.data.requestId,
							result,
							type: 'shared-document-proposal-completed',
						}),
					);
				} catch {
					void this.dispose();
				}
			});
	};

	readonly #handleClose = (): void => {
		void this.dispose();
	};
}
