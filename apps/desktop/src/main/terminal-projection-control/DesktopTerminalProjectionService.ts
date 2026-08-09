import {
	ProjectedAttachmentBootstrapSchema,
	type ProjectedAttachmentBootstrap,
} from '@orchbee/workbench-client-protocol';
import {
	AttachTerminalSchema,
	AttachmentAcknowledgedSchema,
	AttachmentCapabilitiesSchema,
	AttachmentIdSchema,
	MessageKind,
	type AttachmentId,
	type ExitResult,
	type ProcessObservation,
	type RuntimeFailure,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';
import type {
	ProjectedTerminalBrokerEffectTarget,
	ProjectedTerminalBrokerLifecycle,
	ProjectedTerminalBrokerMetrics,
} from '../broker';
import type { HostConnection } from '../host';
import type { DesktopTerminalProjectionTarget } from '../terminal-document-control';

export interface DesktopProjectionBrokerHandle {
	readonly metrics: ProjectedTerminalBrokerMetrics;
	dispose(): Promise<void>;
	start(): void;
}

export interface DesktopTerminalProjectionLease {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly metrics: ProjectedTerminalBrokerMetrics;
	readonly rendererPort: MessagePortMain;
	dispose(): Promise<void>;
}

export interface DesktopTerminalProjectionOpenOptions {
	readonly brokerEffectTarget: ProjectedTerminalBrokerEffectTarget;
}

export interface DesktopTerminalProjectionServiceDependencies {
	readonly createBroker: (
		host: HostConnection,
		port: MessagePortMain,
		bootstrap: ProjectedAttachmentBootstrap,
		effectTarget: ProjectedTerminalBrokerEffectTarget,
		lifecycle?: ProjectedTerminalBrokerLifecycle,
	) => DesktopProjectionBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly getProjectionTarget: (
		presentationId: TerminalPresentationId,
	) => DesktopTerminalProjectionTarget | null;
	readonly host: HostConnection;
	readonly randomAttachmentId: () => string;
}

class DesktopTerminalProjectionError extends Error {
	readonly failure: TerminalPresentationFailure;

	constructor(failure: TerminalPresentationFailure) {
		super(failure.code, { cause: failure });
		this.name = 'DesktopTerminalProjectionError';
		this.failure = failure;
	}
}

interface ActiveLease {
	readonly dispose: () => Promise<void>;
	readonly metrics: ProjectedTerminalBrokerMetrics;
	readonly ownsResize: boolean;
}

export class DesktopTerminalProjectionService {
	readonly #activeLeases = new Map<TerminalPresentationId, Set<ActiveLease>>();
	readonly #dependencies: DesktopTerminalProjectionServiceDependencies;
	readonly #detachTasks = new Map<AttachmentId, Promise<void>>();
	readonly #provisionalAttachments = new Map<AttachmentId, TerminalPresentationId>();
	#disposePromise: Promise<void> | undefined;
	readonly #openTails = new Map<TerminalPresentationId, Promise<void>>();
	readonly #pendingDetaches = new Map<TerminalPresentationId, Set<AttachmentId>>();
	readonly #retiredPresentations = new Set<TerminalPresentationId>();
	readonly #retirementGenerations = new Map<TerminalPresentationId, number>();
	readonly #retirementTasks = new Map<TerminalPresentationId, Promise<void>>();
	#disposed = false;

	constructor(dependencies: DesktopTerminalProjectionServiceDependencies) {
		this.#dependencies = dependencies;
	}

	async open(
		presentationId: TerminalPresentationId,
		options: DesktopTerminalProjectionOpenOptions = { brokerEffectTarget: 'desktop' },
	): Promise<DesktopTerminalProjectionLease> {
		if (this.#disposed) throw projectionError('terminal-projection-unavailable', true);
		if (this.#retiredPresentations.has(presentationId)) {
			throw projectionError('terminal-projection-retired', false);
		}
		const previous = this.#openTails.get(presentationId) ?? Promise.resolve();
		const operation = previous.then(async () => this.#openSerialized(presentationId, options));
		const tail = operation.then(
			() => undefined,
			() => undefined,
		);
		this.#openTails.set(presentationId, tail);
		void tail.then(() => {
			if (this.#openTails.get(presentationId) === tail) {
				this.#openTails.delete(presentationId);
			}
		});
		return operation;
	}

	async #openSerialized(
		presentationId: TerminalPresentationId,
		options: DesktopTerminalProjectionOpenOptions,
	): Promise<DesktopTerminalProjectionLease> {
		if (this.#disposed) throw projectionError('terminal-projection-unavailable', true);
		if (this.#retiredPresentations.has(presentationId)) {
			throw projectionError('terminal-projection-retired', false);
		}
		await this.#retryPendingDetaches(presentationId);
		if (this.#disposed) throw projectionError('terminal-projection-unavailable', true);
		if (this.#retiredPresentations.has(presentationId)) {
			throw projectionError('terminal-projection-retired', false);
		}
		const target = this.#dependencies.getProjectionTarget(presentationId);
		if (target === null) throw projectionError('terminal-presentation-not-ready', true);
		const retirementGeneration = this.#retirementGenerations.get(presentationId) ?? 0;
		const attachmentId = AttachmentIdSchema.parse(this.#dependencies.randomAttachmentId());
		const requestResizeLease =
			!target.readOnly &&
			![...(this.#activeLeases.get(presentationId) ?? [])].some((lease) => lease.ownsResize);
		this.#provisionalAttachments.set(attachmentId, presentationId);
		let capabilities: ReturnType<typeof AttachmentCapabilitiesSchema.parse>;
		try {
			capabilities = await this.#dependencies.host.requestControl(
				MessageKind.attachTerminal,
				AttachTerminalSchema.parse({
					attachmentId,
					capabilities: { input: !target.readOnly, read: true, resize: requestResizeLease },
					requestResizeLease,
					terminalId: target.identity.terminalId,
				}),
				AttachmentCapabilitiesSchema,
			);
		} catch (error: unknown) {
			await this.#detachAttachment(presentationId, attachmentId);
			throw error;
		}
		if (this.#isGenerationStale(presentationId, retirementGeneration)) {
			await this.#detachAttachment(presentationId, attachmentId);
			throw projectionError('terminal-projection-retired', false);
		}
		if (
			!capabilitiesMatch(
				attachmentId,
				target.identity,
				!target.readOnly,
				requestResizeLease,
				capabilities,
			)
		) {
			await this.#detachAttachment(presentationId, attachmentId);
			throw projectionError('terminal-projection-capability-rejected', false);
		}
		const currentTarget = this.#dependencies.getProjectionTarget(presentationId);
		if (currentTarget === null || this.#isGenerationStale(presentationId, retirementGeneration)) {
			await this.#detachAttachment(presentationId, attachmentId);
			throw projectionError('terminal-projection-retired', false);
		}
		if (!projectionTargetsMatch(target, currentTarget)) {
			await this.#detachAttachment(presentationId, attachmentId);
			throw projectionError('terminal-projection-unavailable', true);
		}

		let channel:
			| {
					readonly port1: MessagePortMain;
					readonly port2: MessagePortMain;
			  }
			| undefined;
		let broker: DesktopProjectionBrokerHandle | undefined;
		let activeLease: ActiveLease | undefined;
		let cleanupRequested = false;
		try {
			const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
				attachmentId,
				canInput: capabilities.capabilities.input,
				canResize: capabilities.resizeLease,
				creationId: target.identity.creationId,
				hostEpoch: target.identity.hostEpoch,
				terminalId: target.identity.terminalId,
				terminalOutcome: target.terminalOutcome,
			});
			channel = this.#dependencies.createMessageChannel();
			const requestCleanup = (_error: unknown): void => {
				cleanupRequested = true;
				if (activeLease !== undefined) void activeLease.dispose().catch(() => undefined);
			};
			const createdBroker = this.#dependencies.createBroker(
				this.#dependencies.host,
				channel.port2,
				bootstrap,
				options.brokerEffectTarget,
				{ onCleanupRequested: requestCleanup },
			);
			broker = createdBroker;
			let disposalCompleted = false;
			let disposalPromise: Promise<void> | undefined;
			const removeActiveLease = (): void => {
				this.#provisionalAttachments.delete(attachmentId);
				if (activeLease !== undefined) {
					this.#activeLeases.get(presentationId)?.delete(activeLease);
				}
				if (this.#activeLeases.get(presentationId)?.size === 0) {
					this.#activeLeases.delete(presentationId);
				}
			};
			activeLease = {
				dispose: () => {
					if (disposalCompleted) return Promise.resolve();
					if (disposalPromise !== undefined) return disposalPromise;
					const operation = Promise.resolve()
						.then(() => createdBroker.dispose())
						.then(() => {
							disposalCompleted = true;
							removeActiveLease();
						});
					disposalPromise = operation.catch((error: unknown) => {
						disposalPromise = undefined;
						throw error;
					});
					return disposalPromise;
				},
				get metrics() {
					return createdBroker.metrics;
				},
				ownsResize: bootstrap.canResize,
			};
			const leases = this.#activeLeases.get(presentationId) ?? new Set<ActiveLease>();
			leases.add(activeLease);
			this.#activeLeases.set(presentationId, leases);
			if (cleanupRequested) {
				await activeLease.dispose();
				throw projectionError('terminal-projection-cleanup-failed', true);
			}
			if (this.#isGenerationStale(presentationId, retirementGeneration)) {
				await activeLease.dispose();
				throw projectionError('terminal-projection-retired', false);
			}
			broker.start();
			if (cleanupRequested) {
				await activeLease.dispose();
				throw projectionError('terminal-projection-cleanup-failed', true);
			}
			this.#provisionalAttachments.delete(attachmentId);
			return {
				bootstrap,
				dispose: activeLease.dispose,
				get metrics() {
					return createdBroker.metrics;
				},
				rendererPort: channel.port1,
			};
		} catch (error: unknown) {
			let finalError = error;
			try {
				if (activeLease !== undefined) {
					await activeLease.dispose();
				} else if (broker !== undefined) {
					await broker.dispose();
				} else {
					await this.#detachAttachment(presentationId, attachmentId);
				}
			} catch (cleanupError: unknown) {
				finalError = cleanupError;
			}
			closeMessageChannel(channel);
			throw finalError;
		}
	}

	#isGenerationStale(presentationId: TerminalPresentationId, generation: number): boolean {
		return (
			this.#disposed ||
			this.#retiredPresentations.has(presentationId) ||
			(this.#retirementGenerations.get(presentationId) ?? 0) !== generation
		);
	}

	async #detachAttachment(
		presentationId: TerminalPresentationId,
		attachmentId: AttachmentId,
	): Promise<void> {
		const existing = this.#detachTasks.get(attachmentId);
		if (existing !== undefined) return existing;
		const operation = this.#dependencies.host
			.requestControl(MessageKind.detachTerminal, { attachmentId }, AttachmentAcknowledgedSchema)
			.then((acknowledgement) => {
				if (acknowledgement.attachmentId !== attachmentId) {
					throw projectionError('terminal-projection-cleanup-failed', true);
				}
				this.#provisionalAttachments.delete(attachmentId);
				const pending = this.#pendingDetaches.get(presentationId);
				pending?.delete(attachmentId);
				if (pending?.size === 0) this.#pendingDetaches.delete(presentationId);
			})
			.catch((error: unknown) => {
				const pending = this.#pendingDetaches.get(presentationId) ?? new Set<AttachmentId>();
				pending.add(attachmentId);
				this.#pendingDetaches.set(presentationId, pending);
				throw error instanceof DesktopTerminalProjectionError
					? error
					: projectionError('terminal-projection-cleanup-failed', true);
			});
		this.#detachTasks.set(attachmentId, operation);
		void operation.then(
			() => {
				if (this.#detachTasks.get(attachmentId) === operation) {
					this.#detachTasks.delete(attachmentId);
				}
			},
			() => {
				if (this.#detachTasks.get(attachmentId) === operation) {
					this.#detachTasks.delete(attachmentId);
				}
			},
		);
		return operation;
	}

	async #retryPendingDetaches(presentationId: TerminalPresentationId): Promise<void> {
		const pending = this.#pendingDetaches.get(presentationId);
		if (pending === undefined) return;
		await Promise.all(
			[...pending].map(async (attachmentId) =>
				this.#detachAttachment(presentationId, attachmentId),
			),
		);
	}

	getMetrics(): readonly ProjectedTerminalBrokerMetrics[] {
		return [...this.#activeLeases.values()].flatMap((leases) =>
			[...leases].map((lease) => lease.metrics),
		);
	}

	retirePresentation(presentationId: TerminalPresentationId): Promise<void> {
		if (!this.#retiredPresentations.has(presentationId)) {
			this.#retiredPresentations.add(presentationId);
			this.#retirementGenerations.set(
				presentationId,
				(this.#retirementGenerations.get(presentationId) ?? 0) + 1,
			);
		}
		const existing = this.#retirementTasks.get(presentationId);
		if (existing !== undefined) return existing;
		const operation = this.#retireSerialized(presentationId);
		this.#retirementTasks.set(presentationId, operation);
		void operation.then(
			() => this.#clearRetirementTask(presentationId, operation),
			() => this.#clearRetirementTask(presentationId, operation),
		);
		return operation;
	}

	async #retireSerialized(presentationId: TerminalPresentationId): Promise<void> {
		await this.#openTails.get(presentationId);
		let failed = false;
		let firstFailure: unknown;
		const leases = [...(this.#activeLeases.get(presentationId) ?? [])];
		try {
			await Promise.all(leases.map(async (lease) => lease.dispose()));
		} catch (error: unknown) {
			failed = true;
			firstFailure = error;
		}
		try {
			await this.#retryPendingDetaches(presentationId);
		} catch (error: unknown) {
			if (!failed) {
				failed = true;
				firstFailure = error;
			}
		}
		if (failed) throw firstFailure;
	}

	#clearRetirementTask(presentationId: TerminalPresentationId, task: Promise<void>): void {
		if (this.#retirementTasks.get(presentationId) === task) {
			this.#retirementTasks.delete(presentationId);
		}
	}

	async dispose(): Promise<void> {
		this.#disposed = true;
		if (this.#disposePromise !== undefined) return this.#disposePromise;
		const operation = this.#disposeAll();
		this.#disposePromise = operation.catch((error: unknown) => {
			this.#disposePromise = undefined;
			throw error;
		});
		return this.#disposePromise;
	}

	async #disposeAll(): Promise<void> {
		await Promise.all(this.#openTails.values());
		let failed = false;
		let firstFailure: unknown;
		const leases = [...this.#activeLeases.values()].flatMap((active) => [...active]);
		try {
			await Promise.all(leases.map(async (lease) => lease.dispose()));
		} catch (error: unknown) {
			failed = true;
			firstFailure = error;
		}
		try {
			await Promise.all(
				[...this.#pendingDetaches.keys()].map(async (presentationId) =>
					this.#retryPendingDetaches(presentationId),
				),
			);
		} catch (error: unknown) {
			if (!failed) {
				failed = true;
				firstFailure = error;
			}
		}
		if (failed) throw firstFailure;
	}
}

function closeMessageChannel(
	channel:
		| {
				readonly port1: MessagePortMain;
				readonly port2: MessagePortMain;
		  }
		| undefined,
): void {
	if (channel === undefined) return;
	try {
		channel.port1.close();
	} catch {
		// Channel closure is best effort after the Host attachment has an owner.
	}
	try {
		channel.port2.close();
	} catch {
		// Channel closure is best effort after the Host attachment has an owner.
	}
}

function capabilitiesMatch(
	attachmentId: ReturnType<typeof AttachmentIdSchema.parse>,
	identity: TerminalIdentity,
	canInput: boolean,
	requestedResizeLease: boolean,
	capabilities: ReturnType<typeof AttachmentCapabilitiesSchema.parse>,
): boolean {
	return (
		capabilities.attachmentId === attachmentId &&
		capabilities.terminalId === identity.terminalId &&
		capabilities.capabilities.read &&
		capabilities.capabilities.input === canInput &&
		capabilities.capabilities.resize === requestedResizeLease &&
		capabilities.resizeLease === requestedResizeLease
	);
}

function projectionTargetsMatch(
	left: DesktopTerminalProjectionTarget,
	right: DesktopTerminalProjectionTarget,
): boolean {
	return (
		identitiesMatch(left.identity, right.identity) &&
		left.readOnly === right.readOnly &&
		terminalOutcomesMatch(left.terminalOutcome, right.terminalOutcome)
	);
}

function terminalOutcomesMatch(
	left: DesktopTerminalProjectionTarget['terminalOutcome'],
	right: DesktopTerminalProjectionTarget['terminalOutcome'],
): boolean {
	if (left === null || right === null || left.kind !== right.kind) return left === right;
	return left.kind === 'exited'
		? right.kind === 'exited' && terminalExitsMatch(left.event, right.event)
		: right.kind === 'failed' && terminalFailuresMatch(left.event, right.event);
}

function terminalFailuresMatch(left: TerminalFailed, right: TerminalFailed): boolean {
	return (
		left.finalRevision === right.finalRevision &&
		identitiesMatch(left.identity, right.identity) &&
		runtimeFailuresMatch(left.failure, right.failure) &&
		processObservationsMatch(left.process, right.process)
	);
}

function runtimeFailuresMatch(left: RuntimeFailure, right: RuntimeFailure): boolean {
	return left.kind === right.kind && left.stage === right.stage;
}

function processObservationsMatch(left: ProcessObservation, right: ProcessObservation): boolean {
	if (left.kind !== right.kind) return false;
	return left.kind === 'observed'
		? right.kind === 'observed' && exitResultsMatch(left.result, right.result)
		: right.kind === 'unavailable' && runtimeFailuresMatch(left.failure, right.failure);
}

function identitiesMatch(left: TerminalIdentity, right: TerminalIdentity): boolean {
	return (
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function terminalExitsMatch(left: TerminalExited, right: TerminalExited): boolean {
	return (
		left.finalRevision === right.finalRevision &&
		left.identity.creationId === right.identity.creationId &&
		left.identity.hostEpoch === right.identity.hostEpoch &&
		left.identity.terminalId === right.identity.terminalId &&
		exitResultsMatch(left.exit, right.exit)
	);
}

function exitResultsMatch(left: ExitResult, right: ExitResult): boolean {
	if (left.kind !== right.kind) return false;
	switch (left.kind) {
		case 'exit-code':
			return right.kind === 'exit-code' && left.code === right.code;
		case 'signal':
			return right.kind === 'signal' && left.signal === right.signal;
		case 'unknown':
			return right.kind === 'unknown' && left.value === right.value;
	}
}

function projectionError(code: string, retryable: boolean): DesktopTerminalProjectionError {
	return new DesktopTerminalProjectionError({ code, retryable });
}
