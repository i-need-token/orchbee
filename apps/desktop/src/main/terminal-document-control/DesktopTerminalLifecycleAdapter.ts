import {
	TerminalLaunchSchema,
	TerminalPhase,
	TerminalThemeStateSchema,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
	type TerminalLaunch,
	type TerminalSummary,
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationCommand,
	TerminalPresentationEvent,
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench-client-protocol';

import {
	TerminalBindingIdSchema,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStoreLoadResult,
} from '../recovery';
import type { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import type {
	TerminalBindingLifecycleOutcome,
	TerminalCloseOutcome,
	TerminalExitDisposition,
} from '../terminal-lifetime';
type ProvisionCommand = Extract<
	TerminalPresentationCommand,
	{ readonly type: 'terminal-provision-requested' }
>;
type RestoreCommand = Extract<
	TerminalPresentationCommand,
	{ readonly type: 'terminal-restore-requested' }
>;
type CloseCommand = Extract<
	TerminalPresentationCommand,
	{ readonly type: 'terminal-close-requested' }
>;
type TerminalOperationResult = Exclude<
	TerminalPresentationEvent,
	{ readonly type: 'terminal-exited' | 'terminal-failed' | 'terminal-restore-started' }
>;

const MAX_BUFFERED_EXIT_EVENTS = 64;

class TerminalExitBufferBackpressureError extends Error {
	readonly code = 'terminal-exit-buffer-full';

	constructor() {
		super('Terminal exit admission is temporarily full; retry the exit event.');
		this.name = 'TerminalExitBufferBackpressureError';
	}
}

export type DesktopTerminalExitRecordResult =
	| { readonly kind: 'ignored'; readonly reason?: 'closed' }
	| {
			readonly finalRevision: TerminalExited['finalRevision'];
			readonly kind: 'durable';
			readonly presentationId: TerminalPresentationId;
	  }
	| {
			readonly finalRevision: TerminalExited['finalRevision'];
			readonly kind: 'committed';
			readonly presentationId: TerminalPresentationId;
	  }
	| {
			/** Compatibility result for older runtime adapters; Main still commits it. */
			readonly kind: 'published';
			readonly presentationId: TerminalPresentationId;
	  }
	| {
			readonly code:
				| 'terminal-exit-association-pending'
				| 'terminal-exit-buffer-full'
				| 'terminal-exit-persistence-failed';
			readonly kind: 'retryable';
			readonly reason: 'association' | 'buffer-full' | 'durability';
	  }
	| {
			readonly code: 'terminal-exit-identity-unknown';
			readonly kind: 'unattributed';
			readonly reason: 'unknown-identity';
	  };

export interface DesktopTerminalLifecycleOperations {
	execute(command: TerminalPresentationCommand): Promise<TerminalOperationResult>;
	recordFailure(event: TerminalFailed): Promise<TerminalPresentationId | null>;
	/** Commit the Main-owned publication only after the presentation event was accepted. */
	commitExitPublication?(event: TerminalExited): boolean;
	/** Release a Main-owned retry copy from the adapter's association buffer. */
	releasePendingExit?(event: TerminalExited): void;
}

export interface DesktopTerminalBindingLifecyclePort {
	reconcile(): Promise<TerminalBindingLifecycleOutcome>;
	recordExit(event: TerminalExited): Promise<TerminalExitDisposition>;
	requestClose(
		retireProjection: () => Promise<void>,
		disposition: 'remove' | 'stop',
	): Promise<TerminalCloseOutcome>;
}

export interface DesktopTerminalLifecycleAdapterDependencies {
	readonly allocateFreshBindingId: () => string;
	readonly createBindingLifecycle: (
		bindingId: TerminalBindingId,
		launch: TerminalLaunch,
		presentationId: TerminalPresentationId,
		onIdentityObserved: (identity: TerminalIdentity) => void,
	) => DesktopTerminalBindingLifecyclePort;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly loadBindings: () => Promise<TerminalBindingStoreLoadResult>;
	readonly onIdentityObserved?: (identity: TerminalIdentity) => void;
	readonly onPresentationRetired?: (presentationId: TerminalPresentationId) => void;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
	readonly resolveTerminalLaunch: () => TerminalLaunch | Promise<TerminalLaunch>;
}

interface BindingEntry {
	readonly bindingId: TerminalBindingId;
	readonly closingAtInitialization: boolean;
	readonly launch: TerminalLaunch;
	readonly lifecycle: DesktopTerminalBindingLifecyclePort;
	identity: TerminalIdentity | null;
	operationTail: Promise<void>;
	readonly presentationId: TerminalPresentationId;
	exitPersistencePending: boolean;
	exitPublicationPending: boolean;
	exitRecordingRetryPending: boolean;
	exitRetirementPending: boolean;
	publishedExitRevision: TerminalExited['finalRevision'] | null;
	restorable: boolean;
	retired: boolean;
	ready: boolean;
	startRequired: boolean;
	summary: TerminalSummary | null;
	theme: TerminalThemeState;
	terminalExit: TerminalExited | null;
	terminalFailure: TerminalFailed | null;
}

interface PendingOutcomeWaiter {
	readonly reject: (error: unknown) => void;
	readonly resolve: (presentationId: TerminalPresentationId | null) => void;
}

interface PendingExit {
	event: TerminalExited;
	readonly waiters: PendingOutcomeWaiter[];
}

interface PendingFailure {
	event: TerminalFailed;
	readonly waiters: PendingOutcomeWaiter[];
}

export type DesktopTerminalLifecycleAdapterInitialization =
	| { readonly kind: 'ready' }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  }
	| { readonly kind: 'conflict' };

export interface DesktopTerminalProjectionTarget {
	readonly identity: TerminalIdentity;
	readonly readOnly: boolean;
	readonly startRequired: boolean;
	readonly theme: TerminalThemeState;
	readonly terminalOutcome:
		| { readonly event: TerminalExited; readonly kind: 'exited' }
		| { readonly event: TerminalFailed; readonly kind: 'failed' }
		| null;
}

export interface DesktopRecoveredTerminalPresentation {
	readonly launchCwd: string | null;
	readonly presentationId: TerminalPresentationId;
}

interface DesktopTerminalBindingDrainFailure {
	readonly failure: TerminalPresentationFailure;
	readonly presentationId: TerminalPresentationId;
}

export interface DesktopTerminalBindingDrainResult {
	readonly closedCount: number;
	readonly failures: readonly DesktopTerminalBindingDrainFailure[];
}

export class DesktopTerminalLifecycleAdapter implements DesktopTerminalLifecycleOperations {
	#disposed = false;
	readonly #pendingLaunches = new Map<TerminalPresentationId, Promise<TerminalLaunch>>();
	readonly #allocateFreshBindingId: () => string;
	readonly #createBindingLifecycle: DesktopTerminalLifecycleAdapterDependencies['createBindingLifecycle'];
	readonly #entries = new Map<TerminalPresentationId, BindingEntry>();
	#initialization: Promise<DesktopTerminalLifecycleAdapterInitialization> | undefined;
	#initializationComplete = false;
	readonly #loadBindings: DesktopTerminalLifecycleAdapterDependencies['loadBindings'];
	readonly #onIdentityObserved: DesktopTerminalLifecycleAdapterDependencies['onIdentityObserved'];
	readonly #onPresentationRetired: DesktopTerminalLifecycleAdapterDependencies['onPresentationRetired'];
	readonly #pendingExitEvents = new Map<string, PendingExit>();
	readonly #pendingFailureEvents = new Map<string, PendingFailure>();
	readonly #registry: DesktopTerminalBindingRegistry;
	readonly #retiredPresentations = new Set<TerminalPresentationId>();
	readonly #retireProjection: DesktopTerminalLifecycleAdapterDependencies['retireProjection'];
	readonly #resolveTerminalLaunch: DesktopTerminalLifecycleAdapterDependencies['resolveTerminalLaunch'];

	constructor(dependencies: DesktopTerminalLifecycleAdapterDependencies) {
		this.#allocateFreshBindingId = dependencies.allocateFreshBindingId;
		this.#createBindingLifecycle = dependencies.createBindingLifecycle;
		this.#loadBindings = dependencies.loadBindings;
		this.#onIdentityObserved = dependencies.onIdentityObserved;
		this.#onPresentationRetired = dependencies.onPresentationRetired;
		this.#registry = dependencies.registry;
		this.#retireProjection = dependencies.retireProjection;
		this.#resolveTerminalLaunch = dependencies.resolveTerminalLaunch;
	}

	initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		this.#initialization ??= this.#initialize();
		return this.#initialization;
	}

	dispose(): void {
		this.#disposed = true;
		this.#initializationComplete = true;
		this.#discardPendingExitEvents();
		this.#discardPendingFailureEvents();
	}

	getRecoveredPresentations(): readonly DesktopRecoveredTerminalPresentation[] {
		return [...this.#entries.values()]
			.filter((entry) => !entry.retired && !entry.closingAtInitialization)
			.sort((left, right) => left.presentationId.localeCompare(right.presentationId))
			.map((entry) => ({
				launchCwd: entry.launch.currentDirectory,
				presentationId: entry.presentationId,
			}));
	}

	get bindingCount(): number {
		return this.#registry.size;
	}

	async closeBindingsNotReferencedBy(
		referencedPresentationIds: ReadonlySet<TerminalPresentationId>,
	): Promise<DesktopTerminalBindingDrainResult> {
		const initialization = await this.initialize();
		if (initialization.kind !== 'ready') {
			const mappedFailure =
				initialization.kind === 'unavailable'
					? storageFailure(initialization.reason)
					: failure('terminal-binding-registry-conflict', false);
			return {
				closedCount: 0,
				failures: this.#registry
					.getSnapshot()
					.filter((registration) => !referencedPresentationIds.has(registration.presentationId))
					.map((registration) => ({
						failure: mappedFailure,
						presentationId: registration.presentationId,
					})),
			};
		}
		const targets = this.#registry
			.getSnapshot()
			.filter((registration) => !referencedPresentationIds.has(registration.presentationId));
		const outcomes = await Promise.all(
			targets.map(async (registration): Promise<DesktopTerminalBindingDrainFailure | null> => {
				const entry = this.#entries.get(registration.presentationId);
				if (entry === undefined) {
					return {
						failure: failure('terminal-binding-runtime-unavailable', false),
						presentationId: registration.presentationId,
					};
				}
				const outcome = await this.#enqueue(entry, async () => {
					if (entry.retired) return { kind: 'closed' } as const;
					return entry.lifecycle.requestClose(
						() => this.#retireProjection(entry.presentationId),
						'remove',
					);
				});
				if (outcome.kind !== 'closed') {
					return { failure: closeFailure(outcome), presentationId: entry.presentationId };
				}
				if (!entry.retired && !this.#retireClosedEntry(entry)) {
					return {
						failure: failure('terminal-binding-registry-conflict', false),
						presentationId: entry.presentationId,
					};
				}
				return null;
			}),
		);
		const failures = outcomes.filter(
			(outcome): outcome is DesktopTerminalBindingDrainFailure => outcome !== null,
		);
		return { closedCount: targets.length - failures.length, failures };
	}

	getProjectionTarget(
		presentationId: TerminalPresentationId,
	): DesktopTerminalProjectionTarget | null {
		const entry = this.#entries.get(presentationId);
		if (entry?.ready !== true || entry.retired || entry.identity === null) {
			return null;
		}
		let terminalOutcome: DesktopTerminalProjectionTarget['terminalOutcome'] = null;
		if (entry.terminalFailure !== null) {
			terminalOutcome = { event: entry.terminalFailure, kind: 'failed' };
		} else if (entry.terminalExit !== null) {
			terminalOutcome = { event: entry.terminalExit, kind: 'exited' };
		}
		return {
			identity: entry.identity,
			readOnly:
				entry.terminalExit !== null ||
				entry.terminalFailure !== null ||
				entry.exitPublicationPending ||
				entry.exitRetirementPending ||
				entry.summary?.phase === TerminalPhase.finishing ||
				entry.summary?.phase === TerminalPhase.containingFailure ||
				entry.summary?.phase === TerminalPhase.exited ||
				entry.summary?.phase === TerminalPhase.failed,
			startRequired: entry.startRequired,
			terminalOutcome,
			theme: entry.theme,
		};
	}

	/** Retain the newest complete Host theme observed by an active projection broker. */
	observeTerminalTheme(
		presentationId: TerminalPresentationId,
		candidate: TerminalThemeState,
	): boolean {
		const entry = this.#entries.get(presentationId);
		if (entry?.ready !== true || entry.retired) return false;
		const parsed = TerminalThemeStateSchema.safeParse(candidate);
		if (!parsed.success) return false;
		const currentRevision = BigInt(entry.theme.revision);
		const candidateRevision = BigInt(parsed.data.revision);
		if (candidateRevision < currentRevision) return false;
		if (
			candidateRevision === currentRevision &&
			!terminalThemeStatesMatch(entry.theme, parsed.data)
		) {
			return false;
		}
		entry.theme = parsed.data;
		if (entry.summary !== null) entry.summary = { ...entry.summary, theme: parsed.data };
		return true;
	}

	markTerminalStarted(presentationId: TerminalPresentationId, identity: TerminalIdentity): boolean {
		const entry = this.#entries.get(presentationId);
		if (
			entry?.ready !== true ||
			entry.retired ||
			entry.identity === null ||
			!identitiesEqual(entry.identity, identity)
		) {
			return false;
		}
		entry.startRequired = false;
		return true;
	}

	getDurableTerminalExit(presentationId: TerminalPresentationId): TerminalExited | null {
		const entry = this.#entries.get(presentationId);
		if (
			entry?.ready !== true ||
			entry.retired ||
			entry.identity === null ||
			entry.exitPersistencePending
		) {
			return null;
		}
		return entry.terminalExit;
	}

	getTerminalFailure(presentationId: TerminalPresentationId): TerminalFailed | null {
		const entry = this.#entries.get(presentationId);
		return entry?.ready === true && !entry.retired ? entry.terminalFailure : null;
	}

	releasePendingExit(event: TerminalExited): void {
		const pending = this.#pendingExitEvents.get(identityKey(event.identity));
		if (
			pending !== undefined &&
			pending.waiters.length === 0 &&
			BigInt(pending.event.finalRevision) <= BigInt(event.finalRevision)
		) {
			this.#pendingExitEvents.delete(identityKey(event.identity));
		}
	}

	findPresentationByIdentity(identity: TerminalIdentity): TerminalPresentationId | null {
		for (const entry of this.#entries.values()) {
			if (!entry.retired && identitiesEqual(entry.identity, identity)) return entry.presentationId;
		}
		return null;
	}

	recordExit(event: TerminalExited): Promise<TerminalPresentationId | null> {
		const presentationId = this.findPresentationByIdentity(event.identity);
		if (presentationId === null) {
			return this.#canBufferOutcomeEvent() ? this.#bufferExitEvent(event) : Promise.resolve(null);
		}
		const entry = this.#entries.get(presentationId);
		if (entry === undefined || entry.retired) {
			return this.#canBufferOutcomeEvent() ? this.#bufferExitEvent(event) : Promise.resolve(null);
		}
		if (!this.#observeExit(entry, event)) return Promise.resolve(null);
		return this.#enqueue(entry, async () => this.#recordObservedExit(entry, presentationId));
	}

	recordFailure(event: TerminalFailed): Promise<TerminalPresentationId | null> {
		const presentationId = this.findPresentationByIdentity(event.identity);
		if (presentationId === null) {
			return this.#canBufferOutcomeEvent()
				? this.#bufferFailureEvent(event)
				: Promise.resolve(null);
		}
		const entry = this.#entries.get(presentationId);
		if (entry === undefined || entry.retired) {
			return this.#canBufferOutcomeEvent()
				? this.#bufferFailureEvent(event)
				: Promise.resolve(null);
		}
		return Promise.resolve(this.#observeFailure(entry, event) ? presentationId : null);
	}

	async recordExitOutcome(event: TerminalExited): Promise<DesktopTerminalExitRecordResult> {
		try {
			const presentationId = this.findPresentationByIdentity(event.identity);
			if (presentationId === null) {
				if (!this.#canBufferOutcomeEvent()) {
					return {
						code: 'terminal-exit-identity-unknown',
						kind: 'unattributed',
						reason: 'unknown-identity',
					};
				}
				this.#admitExitEvent(event);
				return {
					code: 'terminal-exit-association-pending',
					kind: 'retryable',
					reason: 'association',
				};
			}
			const recordedPresentationId = await this.recordExit(event);
			if (recordedPresentationId !== null) {
				const entry = this.#entryForIdentity(event.identity);
				return {
					finalRevision: entry?.terminalExit?.finalRevision ?? event.finalRevision,
					kind: 'durable',
					presentationId: recordedPresentationId,
				};
			}
			const entry = this.#entryForIdentity(event.identity);
			if (
				entry !== undefined &&
				!entry.retired &&
				entry.exitPersistencePending &&
				entry.terminalExit !== null &&
				BigInt(entry.terminalExit.finalRevision) >= BigInt(event.finalRevision)
			) {
				return {
					code: 'terminal-exit-persistence-failed',
					kind: 'retryable',
					reason: 'durability',
				};
			}
			if (entry === undefined || entry.retired) {
				if (this.#retiredPresentations.has(presentationId)) {
					return { kind: 'ignored', reason: 'closed' };
				}
				return {
					code: 'terminal-exit-identity-unknown',
					kind: 'unattributed',
					reason: 'unknown-identity',
				};
			}
			if (
				entry.publishedExitRevision !== null &&
				BigInt(entry.publishedExitRevision) >= BigInt(event.finalRevision)
			) {
				return {
					finalRevision: entry.publishedExitRevision,
					kind: 'committed',
					presentationId: entry.presentationId,
				};
			}
			return { kind: 'ignored' };
		} catch (error: unknown) {
			if (error instanceof TerminalExitBufferBackpressureError) {
				return exitBufferBackpressure();
			}
			return {
				code: 'terminal-exit-persistence-failed',
				kind: 'retryable',
				reason: 'durability',
			};
		}
	}

	async execute(command: TerminalPresentationCommand): Promise<TerminalOperationResult> {
		const initialization = await this.initialize();
		if (this.#disposed)
			return operationFailed(command, failure('terminal-binding-runtime-unavailable', false));
		if (initialization.kind !== 'ready') {
			const mappedFailure =
				initialization.kind === 'unavailable'
					? storageFailure(initialization.reason)
					: failure('terminal-binding-registry-conflict', false);
			return operationFailed(command, mappedFailure);
		}
		switch (command.type) {
			case 'terminal-provision-requested':
				return this.#executeProvision(command);
			case 'terminal-restore-requested':
				return this.#executeRestore(command);
			case 'terminal-close-requested':
				return this.#executeClose(command);
		}
	}

	async #initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		try {
			const result = await this.#initializeEntries();
			return this.#completeInitialization(result);
		} catch (error) {
			this.#initializationComplete = true;
			this.#discardPendingExitEvents();
			this.#discardPendingFailureEvents();
			throw error;
		}
	}

	async #initializeEntries(): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		const loaded = await this.#loadBindings();
		if (loaded.kind === 'unavailable') return loaded;
		if (loaded.kind === 'missing') return { kind: 'ready' };
		const closingEntries: BindingEntry[] = [];
		for (const binding of loaded.document.bindings) {
			const lifecycle = this.#createBindingLifecycle(
				binding.bindingId,
				binding.launch,
				binding.presentationId,
				(identity) => this.#associateIdentity(binding.presentationId, identity),
			);
			const registration = this.#registry.register({
				bindingId: binding.bindingId,
				presentationId: binding.presentationId,
			});
			if (registration.kind === 'rejected') return { kind: 'conflict' };
			const entry = entryFromBinding(binding, lifecycle);
			this.#entries.set(binding.presentationId, entry);
			// Recovered identities are already known before a lifecycle callback can
			// fire. Wake Main's durable spill owner for them as part of startup.
			if (entry.identity !== null) this.#onIdentityObserved?.(entry.identity);
			if (entry.closingAtInitialization) closingEntries.push(entry);
		}
		const closingOutcomes = await Promise.all(
			closingEntries.map(async (entry) => ({ entry, outcome: await entry.lifecycle.reconcile() })),
		);
		for (const { entry, outcome } of closingOutcomes) {
			if (outcome.kind === 'close-completed') {
				if (!this.#retireClosedEntry(entry)) return { kind: 'conflict' };
				continue;
			}
			if (outcome.kind === 'close-pending') continue;
			if (outcome.kind === 'ledger-unavailable') {
				return { kind: 'unavailable', reason: outcome.reason };
			}
			return { kind: 'conflict' };
		}
		return { kind: 'ready' };
	}

	async #executeProvision(command: ProvisionCommand): Promise<TerminalOperationResult> {
		if (this.#retiredPresentations.has(command.presentationId)) {
			return Promise.resolve(
				provisionFailed(command, failure('terminal-presentation-retired', false)),
			);
		}

		let entry = this.#entries.get(command.presentationId);
		if (entry === undefined) {
			if (this.#registry.getByPresentationId(command.presentationId) !== null) {
				return Promise.resolve(
					provisionFailed(command, failure('terminal-binding-runtime-unavailable', false)),
				);
			}
			let launch: TerminalLaunch;
			try {
				let pending = this.#pendingLaunches.get(command.presentationId);
				if (pending === undefined) {
					pending = Promise.resolve(this.#resolveTerminalLaunch());
					this.#pendingLaunches.set(command.presentationId, pending);
				}
				try {
					launch = resolveLaunch(await pending, command.launchCwd);
				} finally {
					if (this.#pendingLaunches.get(command.presentationId) === pending)
						this.#pendingLaunches.delete(command.presentationId);
				}
			} catch {
				return provisionFailed(command, failure('terminal-launch-unavailable', true));
			}
			// Discovery is asynchronous. A close or shutdown retires its authority to create.
			if (this.#disposed)
				return provisionFailed(command, failure('terminal-binding-runtime-unavailable', false));
			if (this.#retiredPresentations.has(command.presentationId))
				return provisionFailed(command, failure('terminal-presentation-retired', false));
			if (this.#entries.has(command.presentationId)) return this.#executeProvision(command);
			const bindingId = TerminalBindingIdSchema.safeParse(this.#allocateFreshBindingId());
			if (!bindingId.success) {
				return Promise.resolve(
					provisionFailed(command, failure('terminal-binding-allocation-unavailable', true)),
				);
			}
			const lifecycle = this.#createBindingLifecycle(
				bindingId.data,
				launch,
				command.presentationId,
				(identity) => this.#associateIdentity(command.presentationId, identity),
			);
			const registration = this.#registry.register({
				bindingId: bindingId.data,
				presentationId: command.presentationId,
			});
			if (registration.kind === 'rejected') {
				return Promise.resolve(
					provisionFailed(
						command,
						failure(
							registration.reason === 'binding-id-conflict'
								? 'terminal-binding-allocation-conflict'
								: 'terminal-presentation-conflict',
							registration.reason === 'binding-id-conflict',
						),
					),
				);
			}
			entry = {
				bindingId: bindingId.data,
				closingAtInitialization: false,
				identity: null,
				launch,
				lifecycle,
				operationTail: Promise.resolve(),
				presentationId: command.presentationId,
				exitPersistencePending: false,
				exitPublicationPending: false,
				exitRecordingRetryPending: false,
				exitRetirementPending: false,
				publishedExitRevision: null,
				restorable: false,
				retired: false,
				ready: false,
				startRequired: launch.columns === null,
				summary: null,
				theme: initialThemeFromLaunch(launch),
				terminalExit: null,
				terminalFailure: null,
			};
			this.#entries.set(command.presentationId, entry);
		} else if (
			entry.launch.currentDirectory !== (command.launchCwd ?? entry.launch.currentDirectory)
		) {
			return Promise.resolve(provisionFailed(command, failure('terminal-launch-conflict', false)));
		}

		return this.#enqueue(entry, async () => {
			if (entry.retired) {
				return provisionFailed(command, failure('terminal-presentation-retired', false));
			}
			const outcome = await entry.lifecycle.reconcile();
			if (outcome.kind === 'provisioning' || outcome.kind === 'recovered') {
				entry.identity = outcome.identity;
				entry.ready = true;
				entry.restorable = true;
				if (!applyReconciledSummary(entry, outcome.summary)) {
					return provisionFailed(command, failure('terminal-outcome-conflict', false));
				}
				this.#replayPendingFailureEvent(outcome.identity);
				return provisionCompleted(command);
			}
			if (outcome.kind === 'close-completed') {
				const retired = this.#retireClosedEntry(entry);
				return provisionFailed(
					command,
					failure(
						retired ? 'terminal-presentation-retired' : 'terminal-binding-registry-conflict',
						false,
					),
				);
			}
			return provisionFailed(command, recoveryFailure(outcome));
		});
	}

	#executeRestore(command: RestoreCommand): Promise<TerminalOperationResult> {
		if (this.#retiredPresentations.has(command.presentationId)) {
			return Promise.resolve(
				restoreFailed(command, failure('terminal-presentation-retired', false)),
			);
		}
		const entry = this.#entries.get(command.presentationId);
		if (entry === undefined || !entry.restorable) {
			return Promise.resolve(
				restoreFailed(command, failure('terminal-identity-unavailable', false)),
			);
		}
		if (entry.launch.currentDirectory !== (command.launchCwd ?? entry.launch.currentDirectory)) {
			return Promise.resolve(restoreFailed(command, failure('terminal-launch-conflict', false)));
		}
		return this.#enqueue(entry, async () => {
			if (entry.retired || !entry.restorable) {
				return restoreFailed(command, failure('terminal-presentation-retired', false));
			}
			const outcome = await entry.lifecycle.reconcile();
			if (outcome.kind === 'provisioning' || outcome.kind === 'recovered') {
				entry.identity = outcome.identity;
				entry.ready = true;
				entry.restorable = true;
				if (!applyReconciledSummary(entry, outcome.summary)) {
					return restoreFailed(command, failure('terminal-outcome-conflict', false));
				}
				this.#replayPendingFailureEvent(outcome.identity);
				return restoreCompleted(command);
			}
			if (outcome.kind === 'close-completed') {
				const retired = this.#retireClosedEntry(entry);
				return restoreFailed(
					command,
					failure(
						retired ? 'terminal-presentation-retired' : 'terminal-binding-registry-conflict',
						false,
					),
				);
			}
			return restoreFailed(command, recoveryFailure(outcome));
		});
	}

	#executeClose(command: CloseCommand): Promise<TerminalOperationResult> {
		if (this.#retiredPresentations.has(command.presentationId)) {
			return Promise.resolve(closeCompleted(command));
		}
		const entry = this.#entries.get(command.presentationId);
		if (entry === undefined) {
			if (this.#registry.getByPresentationId(command.presentationId) !== null) {
				return Promise.resolve(
					closeFailed(command, failure('terminal-binding-runtime-unavailable', false)),
				);
			}
			this.#retiredPresentations.add(command.presentationId);
			return Promise.resolve(closeCompleted(command));
		}

		return this.#enqueue(entry, async () => {
			if (entry.retired) return closeCompleted(command);
			const outcome = await entry.lifecycle.requestClose(
				() => this.#retireProjection(command.presentationId),
				command.disposition,
			);
			if (outcome.kind !== 'closed') return closeFailed(command, closeFailure(outcome));
			return this.#retireClosedEntry(entry)
				? closeCompleted(command)
				: closeFailed(command, failure('terminal-binding-registry-conflict', false));
		});
	}

	#enqueue<Output>(entry: BindingEntry, operation: () => Promise<Output>): Promise<Output> {
		const result = entry.operationTail.then(operation);
		entry.operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	#observeExit(entry: BindingEntry, event: TerminalExited): boolean {
		if (entry.terminalFailure !== null) return false;
		const eventRevision = BigInt(event.finalRevision);
		const previousExit = entry.terminalExit;
		if (previousExit !== null) {
			if (!exitOutcomesMatch(previousExit, event)) return false;
			const previousRevision = BigInt(previousExit.finalRevision);
			if (eventRevision < previousRevision) return entry.exitRetirementPending;
			if (eventRevision === previousRevision)
				return (
					entry.publishedExitRevision === null ||
					entry.exitPublicationPending ||
					entry.exitRecordingRetryPending ||
					entry.exitRetirementPending ||
					entry.exitPersistencePending
				);
		}

		// A Host exit is authoritative for projection safety. Merge it before
		// queueing any durable lifecycle work so projection reads fail closed.
		entry.terminalExit = event;
		entry.exitPersistencePending = true;
		entry.exitPublicationPending = false;
		return true;
	}

	#observeFailure(entry: BindingEntry, event: TerminalFailed): boolean {
		if (entry.terminalExit !== null) return false;
		const previousFailure = entry.terminalFailure;
		if (previousFailure !== null) return failuresMatch(previousFailure, event);
		entry.terminalFailure = event;
		return true;
	}

	async #recordObservedExit(
		entry: BindingEntry,
		presentationId: TerminalPresentationId,
		allowPublication = true,
	): Promise<TerminalPresentationId | null> {
		if (entry.retired) return null;
		if (entry.exitRetirementPending) {
			this.#retireClosedEntry(entry);
			return null;
		}
		const event = entry.terminalExit;
		if (event === null) return null;
		if (entry.exitRecordingRetryPending && !entry.exitPersistencePending) {
			const disposition = await entry.lifecycle.recordExit(event);
			if (
				disposition.durable === false ||
				disposition.outcome?.kind === 'store-unavailable' ||
				disposition.outcome?.kind === 'pending'
			) {
				entry.exitPersistencePending = true;
				return null;
			}
			entry.exitRecordingRetryPending = false;
			return allowPublication ? this.#offerExitPublication(entry, presentationId) : null;
		}
		if (!entry.exitPersistencePending) {
			return allowPublication ? this.#offerExitPublication(entry, presentationId) : null;
		}
		const eventRevision = BigInt(event.finalRevision);
		const disposition = await entry.lifecycle.recordExit(event);
		const currentExit = entry.terminalExit;
		const isCurrentRevision =
			currentExit !== null && BigInt(currentExit.finalRevision) === eventRevision;
		const isDurable =
			disposition.durable !== false &&
			disposition.outcome?.kind !== 'store-unavailable' &&
			disposition.outcome?.kind !== 'pending';
		if (!isDurable) {
			// The observed exit remains authoritative for read-only projection, but
			// publication is not committed until the durable retry succeeds.
			entry.exitPersistencePending = true;
			entry.exitRecordingRetryPending = true;
			return null;
		}
		if (disposition.handled && disposition.outcome?.kind === 'closed' && !entry.retired) {
			// A closed binding no longer needs an exit record. Retirement has its
			// own retry state when the registry association cannot be removed.
			entry.exitPersistencePending = false;
			if (!this.#retireClosedEntry(entry)) return null;
		} else if (disposition.handled && isCurrentRevision) {
			// A handled close outcome owns the durable exit record only after the
			// close lifecycle has converged. A transport-pending close is handled by
			// the close owner but has no durable exit record yet.
			entry.exitPersistencePending = false;
			entry.exitRecordingRetryPending = false;
		} else if (!disposition.handled && isCurrentRevision) {
			// A missing durable marker is the compatibility behavior of older
			// lifecycle ports: handled:false means recovery completed or had no
			// binding to record. New ports mark unavailable persistence explicitly.
			entry.exitPersistencePending = false;
			entry.exitRecordingRetryPending = false;
		}
		if (!isCurrentRevision || entry.exitRetirementPending) return null;
		if (!allowPublication) {
			entry.exitPublicationPending =
				entry.publishedExitRevision === null || BigInt(entry.publishedExitRevision) < eventRevision;
			return null;
		}
		return this.#offerExitPublication(entry, presentationId);
	}

	#offerExitPublication(
		entry: BindingEntry,
		presentationId: TerminalPresentationId,
	): TerminalPresentationId | null {
		if (
			entry.exitPersistencePending ||
			entry.exitRetirementPending ||
			entry.retired ||
			entry.terminalExit === null ||
			(entry.publishedExitRevision !== null &&
				BigInt(entry.publishedExitRevision) >= BigInt(entry.terminalExit.finalRevision))
		) {
			return null;
		}
		// This is an offer to Main, not a publication commit. Main must call
		// commitExitPublication after its presentation registry accepts the event.
		entry.exitPublicationPending = true;
		return presentationId;
	}

	commitExitPublication(event: TerminalExited): boolean {
		const entry = this.#entryForIdentity(event.identity);
		if (entry === undefined || entry.retired || entry.terminalExit === null) return false;
		if (BigInt(entry.terminalExit.finalRevision) < BigInt(event.finalRevision)) return false;
		if (
			entry.publishedExitRevision !== null &&
			BigInt(entry.publishedExitRevision) >= BigInt(event.finalRevision)
		) {
			return true;
		}
		if (entry.exitPersistencePending || entry.exitRetirementPending) return false;
		entry.publishedExitRevision = event.finalRevision;
		entry.exitPublicationPending = false;
		return true;
	}

	#retireClosedEntry(entry: BindingEntry): boolean {
		const retired = this.#retireEntry(entry);
		if (!retired) entry.exitRetirementPending = true;
		return retired;
	}

	#retireEntry(entry: BindingEntry): boolean {
		if (entry.identity !== null) {
			this.#discardPendingExitEvent(entry.identity);
			this.#discardPendingFailureEvent(entry.identity);
		}
		const removal = this.#registry.remove(entry.presentationId, entry.bindingId);
		if (removal.kind !== 'removed') return false;
		entry.retired = true;
		this.#entries.delete(entry.presentationId);
		this.#retiredPresentations.add(entry.presentationId);
		this.#onPresentationRetired?.(entry.presentationId);
		return true;
	}

	#completeInitialization(
		result: DesktopTerminalLifecycleAdapterInitialization,
	): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		this.#initializationComplete = true;
		if (result.kind !== 'ready') {
			this.#discardPendingExitEvents();
			this.#discardPendingFailureEvents();
			return Promise.resolve(result);
		}
		return Promise.all([this.#replayPendingExitEvents(), this.#replayPendingFailureEvents()]).then(
			() => result,
		);
	}

	#associateIdentity(presentationId: TerminalPresentationId, identity: TerminalIdentity): void {
		const entry = this.#entries.get(presentationId);
		if (entry === undefined || entry.retired) {
			throw new Error('Terminal identity was observed for an unavailable presentation.');
		}
		if (entry.identity !== null && !identitiesEqual(entry.identity, identity)) {
			throw new Error('Terminal identity was observed for a conflicting presentation.');
		}
		entry.identity = identity;
		this.#replayPendingExitEvent(identity);
		this.#replayPendingFailureEvent(identity);
		this.#onIdentityObserved?.(identity);
	}

	#entryForIdentity(identity: TerminalIdentity): BindingEntry | undefined {
		const presentationId = this.findPresentationByIdentity(identity);
		return presentationId === null ? undefined : this.#entries.get(presentationId);
	}

	#canBufferOutcomeEvent(): boolean {
		return (
			!this.#initializationComplete ||
			[...this.#entries.values()].some((entry) => !entry.retired && entry.identity === null)
		);
	}

	#bufferExitEvent(event: TerminalExited): Promise<TerminalPresentationId | null> {
		let pending: PendingExit;
		try {
			pending = this.#admitExitEvent(event);
		} catch (error: unknown) {
			return Promise.reject(error);
		}
		return new Promise<TerminalPresentationId | null>((resolve, reject) => {
			pending.waiters.push({ reject, resolve });
		});
	}

	#admitExitEvent(event: TerminalExited): PendingExit {
		const key = identityKey(event.identity);
		let pending = this.#pendingExitEvents.get(key);
		if (pending === undefined) {
			if (this.#pendingExitEvents.size >= MAX_BUFFERED_EXIT_EVENTS) {
				// Admission is identity-keyed and lossless for every accepted identity:
				// a full buffer rejects only this new identity, never an older event
				// that may still acquire its presentation association.
				throw new TerminalExitBufferBackpressureError();
			}
			pending = { event, waiters: [] };
			this.#pendingExitEvents.set(key, pending);
		} else if (BigInt(event.finalRevision) > BigInt(pending.event.finalRevision)) {
			pending.event = event;
		}
		return pending;
	}

	async #replayPendingExitEvents(): Promise<void> {
		for (const pending of this.#pendingExitEvents.values()) {
			const presentationId = this.findPresentationByIdentity(pending.event.identity);
			if (presentationId !== null) {
				this.#replayPendingExitEvent(pending.event.identity);
				continue;
			}
			if (!this.#canBufferOutcomeEvent()) {
				this.#discardPendingExitEvent(pending.event.identity);
			}
		}
	}

	#replayPendingExitEvent(identity: TerminalIdentity): void {
		const key = identityKey(identity);
		const pending = this.#pendingExitEvents.get(key);
		if (pending === undefined) return;
		this.#pendingExitEvents.delete(key);
		const presentationId = this.findPresentationByIdentity(identity);
		const entry = presentationId === null ? undefined : this.#entries.get(presentationId);
		if (presentationId === null || entry === undefined || entry.retired) {
			this.#resolvePendingOutcome(pending, null);
			return;
		}
		if (!this.#observeExit(entry, pending.event)) {
			this.#resolvePendingOutcome(pending, null);
			return;
		}
		void this.#enqueue(entry, async () =>
			this.#recordObservedExit(entry, presentationId, pending.waiters.length > 0),
		).then(
			(result) => this.#resolvePendingOutcome(pending, result),
			(error) => this.#rejectPendingExit(pending, error),
		);
	}

	#bufferFailureEvent(event: TerminalFailed): Promise<TerminalPresentationId | null> {
		let pending: PendingFailure;
		try {
			pending = this.#admitFailureEvent(event);
		} catch (error: unknown) {
			return Promise.reject(error);
		}
		return new Promise<TerminalPresentationId | null>((resolve, reject) => {
			pending.waiters.push({ reject, resolve });
		});
	}

	#admitFailureEvent(event: TerminalFailed): PendingFailure {
		const key = identityKey(event.identity);
		let pending = this.#pendingFailureEvents.get(key);
		if (pending === undefined) {
			pending = { event, waiters: [] };
			this.#pendingFailureEvents.set(key, pending);
		} else if (BigInt(event.finalRevision) > BigInt(pending.event.finalRevision)) {
			pending.event = event;
		}
		return pending;
	}

	async #replayPendingFailureEvents(): Promise<void> {
		for (const pending of this.#pendingFailureEvents.values()) {
			const presentationId = this.findPresentationByIdentity(pending.event.identity);
			if (presentationId !== null) {
				this.#replayPendingFailureEvent(pending.event.identity);
				continue;
			}
			if (!this.#canBufferOutcomeEvent()) {
				this.#discardPendingFailureEvent(pending.event.identity);
			}
		}
	}

	#replayPendingFailureEvent(identity: TerminalIdentity): void {
		const key = identityKey(identity);
		const pending = this.#pendingFailureEvents.get(key);
		if (pending === undefined) return;
		this.#pendingFailureEvents.delete(key);
		const presentationId = this.findPresentationByIdentity(identity);
		const entry = presentationId === null ? undefined : this.#entries.get(presentationId);
		const result =
			presentationId !== null &&
			entry !== undefined &&
			!entry.retired &&
			this.#observeFailure(entry, pending.event)
				? presentationId
				: null;
		this.#resolvePendingOutcome(pending, result);
	}

	#discardPendingExitEvent(identity: TerminalIdentity): void {
		const key = identityKey(identity);
		const pending = this.#pendingExitEvents.get(key);
		if (pending === undefined) return;
		this.#pendingExitEvents.delete(key);
		this.#resolvePendingOutcome(pending, null);
	}

	#discardPendingFailureEvent(identity: TerminalIdentity): void {
		const pending = this.#pendingFailureEvents.get(identityKey(identity));
		if (pending === undefined) return;
		this.#pendingFailureEvents.delete(identityKey(identity));
		this.#resolvePendingOutcome(pending, null);
	}

	#discardPendingExitEvents(): void {
		for (const pending of this.#pendingExitEvents.values()) {
			this.#resolvePendingOutcome(pending, null);
		}
		this.#pendingExitEvents.clear();
	}

	#discardPendingFailureEvents(): void {
		for (const pending of this.#pendingFailureEvents.values()) {
			this.#resolvePendingOutcome(pending, null);
		}
		this.#pendingFailureEvents.clear();
	}

	#resolvePendingOutcome(
		pending: PendingExit | PendingFailure,
		result: TerminalPresentationId | null,
	): void {
		let first = true;
		for (const waiter of pending.waiters) {
			waiter.resolve(first ? result : null);
			first = false;
		}
	}

	#rejectPendingExit(pending: PendingExit, error: unknown): void {
		let first = true;
		for (const waiter of pending.waiters) {
			if (first) waiter.reject(error);
			else waiter.resolve(null);
			first = false;
		}
	}
}

function entryFromBinding(
	binding: TerminalBinding,
	lifecycle: DesktopTerminalBindingLifecyclePort,
): BindingEntry {
	const identity =
		binding.state.kind === 'retained' ||
		binding.state.kind === 'exited' ||
		binding.state.kind === 'closing'
			? binding.state.identity
			: null;
	const terminalExit: TerminalExited | null =
		binding.state.kind === 'exited'
			? {
					exit: binding.state.exit,
					finalRevision: binding.state.finalRevision,
					identity: binding.state.identity,
				}
			: null;
	return {
		bindingId: binding.bindingId,
		closingAtInitialization: binding.state.kind === 'closing',
		identity,
		launch: binding.launch,
		lifecycle,
		operationTail: Promise.resolve(),
		presentationId: binding.presentationId,
		exitPersistencePending: false,
		exitPublicationPending: false,
		exitRetirementPending: false,
		publishedExitRevision: null,
		exitRecordingRetryPending: false,
		ready: false,
		startRequired: binding.launch.columns === null,
		restorable: binding.state.kind === 'retained' || binding.state.kind === 'exited',
		retired: false,
		summary: null,
		theme: initialThemeFromLaunch(binding.launch),
		terminalExit,
		terminalFailure: null,
	};
}

function terminalExitFromSummary(summary: TerminalSummary | null): TerminalExited | null {
	return summary?.phase === TerminalPhase.exited && summary.exit !== null
		? { exit: summary.exit, finalRevision: summary.revision, identity: summary.identity }
		: null;
}

function terminalFailureFromSummary(summary: TerminalSummary | null): TerminalFailed | null {
	return summary?.phase === TerminalPhase.failed
		? {
				failure: summary.failure,
				finalRevision: summary.revision,
				identity: summary.identity,
				process: summary.process,
			}
		: null;
}

function applyReconciledSummary(entry: BindingEntry, summary: TerminalSummary | null): boolean {
	const summaryFailure = terminalFailureFromSummary(summary);
	if (summaryFailure !== null) {
		if (entry.terminalExit !== null) return false;
		if (entry.terminalFailure !== null && !failuresMatch(entry.terminalFailure, summaryFailure)) {
			return false;
		}
	}
	const summaryExit = terminalExitFromSummary(summary);
	if (summaryExit !== null) {
		if (entry.terminalFailure !== null) return false;
		if (entry.terminalExit !== null && !exitOutcomesMatch(entry.terminalExit, summaryExit)) {
			return false;
		}
	}

	if (summary !== null) {
		const currentThemeRevision = BigInt(entry.theme.revision);
		const summaryThemeRevision = BigInt(summary.theme.revision);
		if (
			summaryThemeRevision === currentThemeRevision &&
			!terminalThemeStatesMatch(entry.theme, summary.theme)
		) {
			return false;
		}
		if (summaryThemeRevision > currentThemeRevision) entry.theme = summary.theme;
		entry.summary =
			summaryThemeRevision < currentThemeRevision ? { ...summary, theme: entry.theme } : summary;
	} else {
		entry.summary = null;
	}
	entry.startRequired =
		entry.startRequired &&
		(summary?.phase === TerminalPhase.awaitingStart ||
			(summary === null && entry.launch.columns === null));
	if (summaryFailure !== null && entry.terminalFailure === null) {
		entry.terminalFailure = summaryFailure;
	}
	if (
		summaryExit !== null &&
		(entry.terminalExit === null ||
			BigInt(summaryExit.finalRevision) >= BigInt(entry.terminalExit.finalRevision))
	) {
		entry.terminalExit = summaryExit;
		entry.exitPersistencePending = false;
		entry.exitPublicationPending = false;
	}
	return true;
}

function initialThemeFromLaunch(launch: TerminalLaunch): TerminalThemeState {
	return TerminalThemeStateSchema.parse({
		operationId: null,
		revision: '0',
		terminal: {
			colorScheme: launch.terminalProfile.colorScheme,
			defaultBackgroundRgb: launch.terminalProfile.defaultBackgroundRgb,
			defaultCursorRgb: launch.terminalProfile.defaultCursorRgb,
			defaultForegroundRgb: launch.terminalProfile.defaultForegroundRgb,
			palette: launch.terminalProfile.palette,
		},
	});
}

function terminalThemeStatesMatch(left: TerminalThemeState, right: TerminalThemeState): boolean {
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

function exitOutcomesMatch(left: TerminalExited, right: TerminalExited): boolean {
	if (left.exit.kind !== right.exit.kind) return false;
	if (left.exit.kind === 'exit-code' && right.exit.kind === 'exit-code') {
		return left.exit.code === right.exit.code;
	}
	if (left.exit.kind === 'signal' && right.exit.kind === 'signal') {
		return left.exit.signal === right.exit.signal;
	}
	return (
		left.exit.kind === 'unknown' &&
		right.exit.kind === 'unknown' &&
		left.exit.value === right.exit.value
	);
}

function failuresMatch(left: TerminalFailed, right: TerminalFailed): boolean {
	return (
		left.finalRevision === right.finalRevision &&
		left.failure.kind === right.failure.kind &&
		left.failure.stage === right.failure.stage &&
		processObservationsMatch(left.process, right.process)
	);
}

function processObservationsMatch(
	left: TerminalFailed['process'],
	right: TerminalFailed['process'],
): boolean {
	if (left.kind !== right.kind) return false;
	if (left.kind === 'unavailable' && right.kind === 'unavailable') {
		return left.failure.kind === right.failure.kind && left.failure.stage === right.failure.stage;
	}
	if (left.kind !== 'observed' || right.kind !== 'observed') return false;
	if (left.result.kind !== right.result.kind) return false;
	if (left.result.kind === 'exit-code' && right.result.kind === 'exit-code') {
		return left.result.code === right.result.code;
	}
	if (left.result.kind === 'signal' && right.result.kind === 'signal') {
		return left.result.signal === right.result.signal;
	}
	return (
		left.result.kind === 'unknown' &&
		right.result.kind === 'unknown' &&
		left.result.value === right.result.value
	);
}

function identitiesEqual(left: TerminalIdentity | null, right: TerminalIdentity): boolean {
	return (
		left !== null &&
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function identityKey(identity: TerminalIdentity): string {
	return `${identity.hostEpoch}:${identity.creationId}:${identity.terminalId}`;
}

function exitBufferBackpressure(): DesktopTerminalExitRecordResult {
	return {
		code: 'terminal-exit-buffer-full',
		kind: 'retryable',
		reason: 'buffer-full',
	};
}

function resolveLaunch(base: TerminalLaunch, launchCwd: string | null): TerminalLaunch {
	return TerminalLaunchSchema.parse({
		...base,
		currentDirectory: launchCwd ?? base.currentDirectory,
	});
}

function recoveryFailure(
	outcome: Exclude<
		TerminalBindingLifecycleOutcome,
		{ readonly kind: 'close-completed' | 'provisioning' | 'recovered' }
	>,
): TerminalPresentationFailure {
	switch (outcome.kind) {
		case 'close-pending':
			return failure('terminal-closing', false);
		case 'conflict':
			return failure('terminal-identity-conflict', false);
		case 'ledger-unavailable':
			return storageFailure(outcome.reason);
		case 'lost':
			return outcome.reason === 'create-response-unavailable'
				? failure('terminal-unavailable', true)
				: failure('terminal-identity-unavailable', false);
		case 'orphaned':
			return failure('terminal-orphan-conflict', false);
	}
}

function closeFailure(
	outcome: Exclude<TerminalCloseOutcome, { readonly kind: 'closed' }>,
): TerminalPresentationFailure {
	switch (outcome.kind) {
		case 'conflict':
			return failure('terminal-identity-conflict', false);
		case 'pending':
			return outcome.reason === 'transport-unavailable'
				? failure('terminal-transport-unavailable', true)
				: failure('terminal-close-pending', true);
		case 'store-unavailable':
			return storageFailure(outcome.reason);
	}
}

function storageFailure(
	reason: 'invalid' | 'read-failed' | 'write-failed',
): TerminalPresentationFailure {
	return reason === 'invalid'
		? failure('terminal-storage-invalid', false)
		: failure('terminal-storage-unavailable', true);
}

function failure(code: string, retryable: boolean): TerminalPresentationFailure {
	return { code, retryable };
}

function provisionCompleted(command: ProvisionCommand): TerminalOperationResult {
	return {
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-provision-completed',
	};
}

function provisionFailed(
	command: ProvisionCommand,
	failureValue: TerminalPresentationFailure,
): TerminalOperationResult {
	return {
		failure: failureValue,
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-provision-failed',
	};
}

function restoreCompleted(command: RestoreCommand): TerminalOperationResult {
	return {
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-restore-completed',
	};
}

function restoreFailed(
	command: RestoreCommand,
	failureValue: TerminalPresentationFailure,
): TerminalOperationResult {
	return {
		failure: failureValue,
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-restore-failed',
	};
}

function closeCompleted(command: CloseCommand): TerminalOperationResult {
	return {
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-close-completed',
	};
}

function closeFailed(
	command: CloseCommand,
	failureValue: TerminalPresentationFailure,
): TerminalOperationResult {
	return {
		failure: failureValue,
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-close-failed',
	};
}

function operationFailed(
	command: TerminalPresentationCommand,
	failureValue: TerminalPresentationFailure,
): TerminalOperationResult {
	switch (command.type) {
		case 'terminal-provision-requested':
			return provisionFailed(command, failureValue);
		case 'terminal-restore-requested':
			return restoreFailed(command, failureValue);
		case 'terminal-close-requested':
			return closeFailed(command, failureValue);
	}
}
