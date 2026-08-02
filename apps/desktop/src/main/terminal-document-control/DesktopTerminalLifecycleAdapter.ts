import {
	TerminalLaunchV1_1Schema,
	TerminalPhase,
	type TerminalExited,
	type TerminalIdentity,
	type TerminalLaunchV1_1,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import type {
	TerminalDocumentControlCommand,
	TerminalDocumentControlOperationResult,
} from '../../shared';
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
import type { TerminalDocumentControlOperations } from './TerminalDocumentControlBroker';

type ProvisionCommand = Extract<
	TerminalDocumentControlCommand,
	{ readonly type: 'terminal-provision-requested' }
>;
type CloseCommand = Extract<
	TerminalDocumentControlCommand,
	{ readonly type: 'terminal-close-requested' }
>;

export interface DesktopTerminalBindingLifecyclePort {
	reconcile(): Promise<TerminalBindingLifecycleOutcome>;
	recordExit(event: TerminalExited): Promise<TerminalExitDisposition>;
	requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome>;
}

export interface DesktopTerminalLifecycleAdapterDependencies {
	readonly allocateFreshBindingId: () => string;
	readonly createBindingLifecycle: (
		bindingId: TerminalBindingId,
		launch: TerminalLaunchV1_1,
		presentationId: TerminalPresentationId,
	) => DesktopTerminalBindingLifecyclePort;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly loadBindings: () => Promise<TerminalBindingStoreLoadResult>;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
	readonly terminalLaunch: TerminalLaunchV1_1;
}

interface BindingEntry {
	readonly bindingId: TerminalBindingId;
	readonly closingAtInitialization: boolean;
	readonly launch: TerminalLaunchV1_1;
	readonly lifecycle: DesktopTerminalBindingLifecyclePort;
	identity: TerminalIdentity | null;
	operationTail: Promise<void>;
	readonly presentationId: TerminalPresentationId;
	retired: boolean;
	ready: boolean;
	summary: TerminalSummary | null;
	terminalExit: TerminalExited | null;
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
	readonly terminalExit: TerminalExited | null;
}

export interface DesktopRecoveredTerminalPresentation {
	readonly launchCwd: string | null;
	readonly presentationId: TerminalPresentationId;
}

export class DesktopTerminalLifecycleAdapter implements TerminalDocumentControlOperations {
	readonly #allocateFreshBindingId: () => string;
	readonly #createBindingLifecycle: DesktopTerminalLifecycleAdapterDependencies['createBindingLifecycle'];
	readonly #entries = new Map<TerminalPresentationId, BindingEntry>();
	#initialization: Promise<DesktopTerminalLifecycleAdapterInitialization> | undefined;
	readonly #loadBindings: DesktopTerminalLifecycleAdapterDependencies['loadBindings'];
	readonly #registry: DesktopTerminalBindingRegistry;
	readonly #retiredPresentations = new Set<TerminalPresentationId>();
	readonly #retireProjection: DesktopTerminalLifecycleAdapterDependencies['retireProjection'];
	readonly #terminalLaunch: TerminalLaunchV1_1;

	constructor(dependencies: DesktopTerminalLifecycleAdapterDependencies) {
		this.#allocateFreshBindingId = dependencies.allocateFreshBindingId;
		this.#createBindingLifecycle = dependencies.createBindingLifecycle;
		this.#loadBindings = dependencies.loadBindings;
		this.#registry = dependencies.registry;
		this.#retireProjection = dependencies.retireProjection;
		this.#terminalLaunch = TerminalLaunchV1_1Schema.parse(dependencies.terminalLaunch);
	}

	initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		this.#initialization ??= this.#initialize();
		return this.#initialization;
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

	getProjectionTarget(
		presentationId: TerminalPresentationId,
	): DesktopTerminalProjectionTarget | null {
		const entry = this.#entries.get(presentationId);
		if (entry?.ready !== true || entry.retired || entry.identity === null) return null;
		return {
			identity: entry.identity,
			readOnly:
				entry.terminalExit !== null ||
				entry.summary?.phase === TerminalPhase.finishing ||
				entry.summary?.phase === TerminalPhase.exited,
			terminalExit: entry.terminalExit,
		};
	}

	findPresentationByIdentity(identity: TerminalIdentity): TerminalPresentationId | null {
		for (const entry of this.#entries.values()) {
			if (!entry.retired && identitiesEqual(entry.identity, identity)) return entry.presentationId;
		}
		return null;
	}

	async recordExit(event: TerminalExited): Promise<TerminalPresentationId | null> {
		const presentationId = this.findPresentationByIdentity(event.identity);
		if (presentationId === null) return null;
		const entry = this.#entries.get(presentationId);
		if (entry === undefined || entry.retired) return null;
		entry.terminalExit = event;
		const disposition = await this.#enqueue(entry, async () => entry.lifecycle.recordExit(event));
		if (disposition.handled && disposition.outcome?.kind === 'closed' && !entry.retired) {
			this.#retireEntry(entry);
		}
		return entry.retired ? null : presentationId;
	}

	async execute(
		command: TerminalDocumentControlCommand,
	): Promise<TerminalDocumentControlOperationResult> {
		const initialization = await this.initialize();
		if (initialization.kind !== 'ready') {
			const mappedFailure =
				initialization.kind === 'unavailable'
					? storageFailure(initialization.reason)
					: failure('terminal-binding-registry-conflict', false);
			return command.type === 'terminal-provision-requested'
				? provisionFailed(command, mappedFailure)
				: closeFailed(command, mappedFailure);
		}
		return command.type === 'terminal-provision-requested'
			? this.#executeProvision(command)
			: this.#executeClose(command);
	}

	async #initialize(): Promise<DesktopTerminalLifecycleAdapterInitialization> {
		const loaded = await this.#loadBindings();
		if (loaded.kind === 'unavailable') return loaded;
		if (loaded.kind === 'missing') return { kind: 'ready' };
		const closingEntries: BindingEntry[] = [];
		for (const binding of loaded.document.bindings) {
			const lifecycle = this.#createBindingLifecycle(
				binding.bindingId,
				binding.launch,
				binding.presentationId,
			);
			const registration = this.#registry.register({
				bindingId: binding.bindingId,
				presentationId: binding.presentationId,
			});
			if (registration.kind === 'rejected') return { kind: 'conflict' };
			const entry = entryFromBinding(binding, lifecycle);
			this.#entries.set(binding.presentationId, entry);
			if (entry.closingAtInitialization) closingEntries.push(entry);
		}
		const closingOutcomes = await Promise.all(
			closingEntries.map(async (entry) => ({ entry, outcome: await entry.lifecycle.reconcile() })),
		);
		for (const { entry, outcome } of closingOutcomes) {
			if (outcome.kind === 'close-completed') {
				if (!this.#retireEntry(entry)) return { kind: 'conflict' };
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

	#executeProvision(command: ProvisionCommand): Promise<TerminalDocumentControlOperationResult> {
		if (this.#retiredPresentations.has(command.presentationId)) {
			return Promise.resolve(
				provisionFailed(command, failure('terminal-presentation-retired', false)),
			);
		}

		const launch = resolveLaunch(this.#terminalLaunch, command.launchCwd);
		let entry = this.#entries.get(command.presentationId);
		if (entry === undefined) {
			if (this.#registry.getByPresentationId(command.presentationId) !== null) {
				return Promise.resolve(
					provisionFailed(command, failure('terminal-binding-runtime-unavailable', false)),
				);
			}
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
				retired: false,
				ready: false,
				summary: null,
				terminalExit: null,
			};
			this.#entries.set(command.presentationId, entry);
		} else if (entry.launch.currentDirectory !== launch.currentDirectory) {
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
				entry.summary = outcome.summary;
				entry.terminalExit = terminalExitFromSummary(outcome.summary);
				return provisionCompleted(command);
			}
			if (outcome.kind === 'close-completed') {
				const retired = this.#retireEntry(entry);
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

	#executeClose(command: CloseCommand): Promise<TerminalDocumentControlOperationResult> {
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
			const outcome = await entry.lifecycle.requestClose(() =>
				this.#retireProjection(command.presentationId),
			);
			if (outcome.kind !== 'closed') return closeFailed(command, closeFailure(outcome));
			return this.#retireEntry(entry)
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

	#retireEntry(entry: BindingEntry): boolean {
		const removal = this.#registry.remove(entry.presentationId, entry.bindingId);
		if (removal.kind !== 'removed') return false;
		entry.retired = true;
		this.#entries.delete(entry.presentationId);
		this.#retiredPresentations.add(entry.presentationId);
		return true;
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
		ready: false,
		retired: false,
		summary: null,
		terminalExit,
	};
}

function terminalExitFromSummary(summary: TerminalSummary | null): TerminalExited | null {
	return summary?.phase === TerminalPhase.exited && summary.exit !== null
		? { exit: summary.exit, finalRevision: summary.revision, identity: summary.identity }
		: null;
}

function identitiesEqual(left: TerminalIdentity | null, right: TerminalIdentity): boolean {
	return (
		left !== null &&
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function resolveLaunch(base: TerminalLaunchV1_1, launchCwd: string | null): TerminalLaunchV1_1 {
	return TerminalLaunchV1_1Schema.parse({
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

function provisionCompleted(command: ProvisionCommand): TerminalDocumentControlOperationResult {
	return {
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-provision-completed',
	};
}

function provisionFailed(
	command: ProvisionCommand,
	failureValue: TerminalPresentationFailure,
): TerminalDocumentControlOperationResult {
	return {
		failure: failureValue,
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-provision-failed',
	};
}

function closeCompleted(command: CloseCommand): TerminalDocumentControlOperationResult {
	return {
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-close-completed',
	};
}

function closeFailed(
	command: CloseCommand,
	failureValue: TerminalPresentationFailure,
): TerminalDocumentControlOperationResult {
	return {
		failure: failureValue,
		operationId: command.operationId,
		presentationId: command.presentationId,
		type: 'terminal-close-failed',
	};
}
