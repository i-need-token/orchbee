import type { MessageEvent, MessagePortMain } from 'electron';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
	type WorkbenchClientSessionBootstrap,
} from '@orchbee/workbench-client-protocol';
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
	#disposeAttachmentsPromise: Promise<void> | undefined;
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

	get bootstrap(): WorkbenchClientSessionBootstrap {
		const documentState =
			this.#handle.bootstrap.kind === 'available'
				? { document: this.#handle.bootstrap.document, kind: 'available' as const }
				: { kind: 'missing' as const };
		return WorkbenchClientSessionBootstrapSchema.parse({
			clientSessionId: this.#handle.clientSessionId,
			defaultCwd: this.#handle.bootstrap.defaultCwd,
			documentState,
			initialPresentationId: this.#handle.bootstrap.initialPresentationId,
			initializationDocument: this.#handle.bootstrap.initializationDocument,
			sequence: this.#handle.bootstrap.sequence,
			terminalHostName: this.#handle.bootstrap.terminalHostName,
			terminalEvents: this.#handle.bootstrap.terminalEvents,
			type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
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
		const event = WorkbenchClientSessionEventSchema.parse(publication);
		this.#port.postMessage(event);
	}

	disposeAttachments(): Promise<void> {
		if (this.#disposeAttachmentsPromise !== undefined) return this.#disposeAttachmentsPromise;
		if (!this.#disposed) {
			this.#disposed = true;
			this.#port.off('message', this.#handleMessage);
			this.#port.off('close', this.#handleClose);
			this.#port.close();
		}
		let disposal: Promise<void>;
		try {
			disposal = this.#disposeAttachmentsCapability();
		} catch (error: unknown) {
			disposal = Promise.reject(error);
		}
		const operation = disposal.catch((error: unknown) => {
			this.#disposeAttachmentsPromise = undefined;
			throw error;
		});
		this.#disposeAttachmentsPromise = operation;
		return operation;
	}

	readonly #handleMessage = (message: MessageEvent): void => {
		if (this.#disposed) return;
		if (message.ports.length !== 0) {
			for (const port of message.ports) port.close();
			void this.dispose().catch(() => undefined);
			return;
		}
		const command = WorkbenchClientSessionCommandSchema.safeParse(message.data);
		if (!command.success) {
			void this.dispose().catch(() => undefined);
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
						WorkbenchClientSessionEventSchema.parse({
							requestId: command.data.requestId,
							result,
							type: 'shared-document-proposal-completed',
						}),
					);
				} catch {
					void this.dispose().catch(() => undefined);
				}
			})
			.catch(() => {
				// Proposal effects may fail after the coordinator has accepted the
				// document (for example, a first detach attempt). Retire this session
				// through the coordinator so its attachment cleanup remains a retry
				// owner instead of becoming an unhandled rejection.
				if (!this.#disposed) void this.#handle.dispose().catch(() => undefined);
			});
	};

	readonly #handleClose = (): void => {
		void this.dispose().catch(() => undefined);
	};
}
