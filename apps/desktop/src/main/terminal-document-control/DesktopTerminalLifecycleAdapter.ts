import { TerminalLaunchV1_1Schema, type TerminalLaunchV1_1 } from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationFailure,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import type {
	TerminalDocumentControlCommand,
	TerminalDocumentControlOperationResult,
} from '../../shared';
import { TerminalBindingIdSchema, type TerminalBindingId } from '../recovery';
import type { DesktopTerminalBindingRegistry } from '../terminal-binding-registry';
import type { TerminalBindingLifecycleOutcome, TerminalCloseOutcome } from '../terminal-lifetime';
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
	requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome>;
}

export interface DesktopTerminalLifecycleAdapterDependencies {
	readonly allocateFreshBindingId: () => string;
	readonly createBindingLifecycle: (
		bindingId: TerminalBindingId,
		launch: TerminalLaunchV1_1,
	) => DesktopTerminalBindingLifecyclePort;
	readonly registry: DesktopTerminalBindingRegistry;
	readonly retireProjection: (presentationId: TerminalPresentationId) => Promise<void>;
	readonly terminalLaunch: TerminalLaunchV1_1;
}

interface BindingEntry {
	readonly bindingId: TerminalBindingId;
	readonly launch: TerminalLaunchV1_1;
	readonly lifecycle: DesktopTerminalBindingLifecyclePort;
	operationTail: Promise<void>;
	readonly presentationId: TerminalPresentationId;
	retired: boolean;
}

export class DesktopTerminalLifecycleAdapter implements TerminalDocumentControlOperations {
	readonly #allocateFreshBindingId: () => string;
	readonly #createBindingLifecycle: DesktopTerminalLifecycleAdapterDependencies['createBindingLifecycle'];
	readonly #entries = new Map<TerminalPresentationId, BindingEntry>();
	readonly #registry: DesktopTerminalBindingRegistry;
	readonly #retiredPresentations = new Set<TerminalPresentationId>();
	readonly #retireProjection: DesktopTerminalLifecycleAdapterDependencies['retireProjection'];
	readonly #terminalLaunch: TerminalLaunchV1_1;

	constructor(dependencies: DesktopTerminalLifecycleAdapterDependencies) {
		this.#allocateFreshBindingId = dependencies.allocateFreshBindingId;
		this.#createBindingLifecycle = dependencies.createBindingLifecycle;
		this.#registry = dependencies.registry;
		this.#retireProjection = dependencies.retireProjection;
		this.#terminalLaunch = TerminalLaunchV1_1Schema.parse(dependencies.terminalLaunch);
	}

	execute(
		command: TerminalDocumentControlCommand,
	): Promise<TerminalDocumentControlOperationResult> {
		return command.type === 'terminal-provision-requested'
			? this.#executeProvision(command)
			: this.#executeClose(command);
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
			const lifecycle = this.#createBindingLifecycle(bindingId.data, launch);
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
				launch,
				lifecycle,
				operationTail: Promise.resolve(),
				presentationId: command.presentationId,
				retired: false,
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

	#enqueue(
		entry: BindingEntry,
		operation: () => Promise<TerminalDocumentControlOperationResult>,
	): Promise<TerminalDocumentControlOperationResult> {
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
