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
	StartTerminalSchema,
	TerminalStartAcceptedSchema,
	type AttachmentId,
	type AttachmentCapabilities,
	type ExitResult,
	type ProcessObservation,
	type RuntimeFailure,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
	type TerminalGeometry,
	type TerminalThemeState,
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
	setInputCapability(input: boolean): Promise<void>;
	start(): void;
}

export interface DesktopTerminalProjectionLease {
	readonly bootstrap: ProjectedAttachmentBootstrap;
	readonly metrics: ProjectedTerminalBrokerMetrics;
	readonly rendererPort: MessagePortMain;
	dispose(): Promise<void>;
	setInputCapability(input: boolean): Promise<void>;
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
		initialCapabilities?: AttachmentCapabilities,
	) => DesktopProjectionBrokerHandle;
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly getProjectionTarget: (
		presentationId: TerminalPresentationId,
	) => DesktopTerminalProjectionTarget | null;
	readonly host: HostConnection;
	readonly markTerminalStarted: (
		presentationId: TerminalPresentationId,
		identity: TerminalIdentity,
	) => boolean;
	readonly observeTerminalTheme: (
		presentationId: TerminalPresentationId,
		theme: TerminalThemeState,
	) => boolean;
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
	readonly performDispose: () => Promise<void>;
	readonly setInputCapability: (input: boolean) => Promise<void>;
	ownsResize: boolean;
}

export class DesktopTerminalProjectionService {
	readonly #activeLeases = new Map<TerminalPresentationId, Set<ActiveLease>>();
	readonly #dependencies: DesktopTerminalProjectionServiceDependencies;
	readonly #detachTasks = new Map<AttachmentId, Promise<void>>();
	readonly #provisionalAttachments = new Map<AttachmentId, TerminalPresentationId>();
	#disposePromise: Promise<void> | undefined;
	readonly #operationTails = new Map<TerminalPresentationId, Promise<void>>();
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
		geometry: TerminalGeometry,
		options: DesktopTerminalProjectionOpenOptions = { brokerEffectTarget: 'desktop' },
	): Promise<DesktopTerminalProjectionLease> {
		if (this.#disposed) throw projectionError('terminal-projection-unavailable', true);
		if (this.#retiredPresentations.has(presentationId)) {
			throw projectionError('terminal-projection-retired', false);
		}
		return this.#enqueueOperation(presentationId, async () =>
			this.#openSerialized(presentationId, geometry, options),
		);
	}

	#enqueueOperation<Result>(
		presentationId: TerminalPresentationId,
		operation: () => Promise<Result>,
	): Promise<Result> {
		const previous = this.#operationTails.get(presentationId) ?? Promise.resolve();
		const result = previous.then(operation);
		const tail = result.then(
			() => undefined,
			() => undefined,
		);
		this.#operationTails.set(presentationId, tail);
		void tail.then(() => {
			if (this.#operationTails.get(presentationId) === tail) {
				this.#operationTails.delete(presentationId);
			}
		});
		return result;
	}

	async #openSerialized(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
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
		let target = this.#dependencies.getProjectionTarget(presentationId);
		if (target === null) throw projectionError('terminal-presentation-not-ready', true);
		const retirementGeneration = this.#retirementGenerations.get(presentationId) ?? 0;
		if (target.startRequired) {
			const accepted = await this.#dependencies.host.requestControl(
				MessageKind.startTerminal,
				StartTerminalSchema.parse({ geometry, terminalId: target.identity.terminalId }),
				TerminalStartAcceptedSchema,
			);
			if (
				!identitiesMatch(accepted.identity, target.identity) ||
				!geometriesMatch(accepted.geometry, geometry) ||
				!this.#dependencies.markTerminalStarted(presentationId, accepted.identity)
			) {
				throw projectionError('terminal-projection-unavailable', true);
			}
			target = this.#dependencies.getProjectionTarget(presentationId);
			if (target === null || target.startRequired) {
				throw projectionError('terminal-projection-unavailable', true);
			}
		}
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
					capabilities: { input: !target.readOnly, read: true, resize: !target.readOnly },
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
				capabilityRevision: capabilities.capabilityRevision,
				creationId: target.identity.creationId,
				hostEpoch: target.identity.hostEpoch,
				terminalId: target.identity.terminalId,
				terminalOutcome: target.terminalOutcome,
				theme: target.theme,
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
				{
					onCleanupRequested: requestCleanup,
					onResizeLeaseChanged: (ownsResize) => {
						if (activeLease === undefined) {
							throw projectionError('terminal-projection-capability-rejected', false);
						}
						activeLease.ownsResize = ownsResize;
					},
					onThemeChanged: (theme) => {
						if (!this.#dependencies.observeTerminalTheme(presentationId, theme)) {
							throw projectionError('terminal-projection-theme-conflict', false);
						}
					},
				},
				capabilities,
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
			const performDispose = (): Promise<void> => {
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
			};
			activeLease = {
				dispose: () => this.#enqueueOperation(presentationId, performDispose),
				get metrics() {
					return createdBroker.metrics;
				},
				ownsResize: bootstrap.canResize,
				performDispose,
				setInputCapability: (input) =>
					this.#enqueueOperation(presentationId, async () => {
						if (
							disposalCompleted ||
							this.#isGenerationStale(presentationId, retirementGeneration)
						) {
							throw projectionError('terminal-projection-unavailable', true);
						}
						await createdBroker.setInputCapability(input);
					}),
			};
			const leases = this.#activeLeases.get(presentationId) ?? new Set<ActiveLease>();
			leases.add(activeLease);
			this.#activeLeases.set(presentationId, leases);
			if (cleanupRequested) {
				await activeLease.performDispose();
				throw projectionError('terminal-projection-cleanup-failed', true);
			}
			if (this.#isGenerationStale(presentationId, retirementGeneration)) {
				await activeLease.performDispose();
				throw projectionError('terminal-projection-retired', false);
			}
			broker.start();
			if (cleanupRequested) {
				await activeLease.performDispose();
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
				setInputCapability: activeLease.setInputCapability,
			};
		} catch (error: unknown) {
			let finalError = error;
			try {
				if (activeLease !== undefined) {
					await activeLease.performDispose();
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
		const operation = this.#enqueueOperation(presentationId, async () =>
			this.#retireSerialized(presentationId),
		);
		this.#retirementTasks.set(presentationId, operation);
		void operation.then(
			() => this.#clearRetirementTask(presentationId, operation),
			() => this.#clearRetirementTask(presentationId, operation),
		);
		return operation;
	}

	async #retireSerialized(presentationId: TerminalPresentationId): Promise<void> {
		let failed = false;
		let firstFailure: unknown;
		const leases = [...(this.#activeLeases.get(presentationId) ?? [])];
		try {
			await this.#disposeLeases(leases);
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

	async #disposeLeases(leases: readonly ActiveLease[]): Promise<void> {
		let failed = false;
		let firstFailure: unknown;
		for (const lease of leases) {
			try {
				await lease.performDispose();
			} catch (error: unknown) {
				if (!failed) {
					failed = true;
					firstFailure = error;
				}
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
		await Promise.all(this.#operationTails.values());
		let failed = false;
		let firstFailure: unknown;
		try {
			await Promise.all(
				[...this.#activeLeases].map(([presentationId, active]) => {
					const leases = [...active];
					return this.#enqueueOperation(presentationId, () => this.#disposeLeases(leases));
				}),
			);
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
	canResize: boolean,
	requestedResizeLease: boolean,
	capabilities: ReturnType<typeof AttachmentCapabilitiesSchema.parse>,
): boolean {
	return (
		capabilities.attachmentId === attachmentId &&
		capabilities.terminalId === identity.terminalId &&
		capabilities.capabilities.read &&
		capabilities.capabilities.input === canInput &&
		capabilities.capabilities.resize === canResize &&
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
		left.startRequired === right.startRequired &&
		terminalThemeStatesMatch(left.theme, right.theme) &&
		terminalOutcomesMatch(left.terminalOutcome, right.terminalOutcome)
	);
}

function terminalThemeStatesMatch(
	left: DesktopTerminalProjectionTarget['theme'],
	right: DesktopTerminalProjectionTarget['theme'],
): boolean {
	return (
		left.operationId === right.operationId &&
		left.revision === right.revision &&
		left.terminal.colorScheme === right.terminal.colorScheme &&
		left.terminal.defaultBackgroundRgb === right.terminal.defaultBackgroundRgb &&
		left.terminal.defaultCursorRgb === right.terminal.defaultCursorRgb &&
		left.terminal.defaultForegroundRgb === right.terminal.defaultForegroundRgb &&
		left.terminal.palette.length === right.terminal.palette.length &&
		left.terminal.palette.every((color, index) => color === right.terminal.palette[index])
	);
}

function geometriesMatch(left: TerminalGeometry, right: TerminalGeometry): boolean {
	return (
		left.cellPixelHeight === right.cellPixelHeight &&
		left.cellPixelWidth === right.cellPixelWidth &&
		left.columns === right.columns &&
		left.rows === right.rows
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
