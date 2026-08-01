import {
	CreateTerminalV1_1Schema,
	CreationIdSchema,
	EmptyControlSchema,
	MessageKind,
	TerminalIdentitySchema,
	TerminalListSchema,
	TerminalLaunchV1_1Schema,
	TerminalPhase,
	type HostEpoch,
	type TerminalExited,
	type TerminalIdentity,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';

import type { HostConnection } from '../host';
import { HostRequestError } from '../host';
import {
	TerminalRecoveryLedgerDocumentSchema,
	type TerminalRecoveryBinding,
	type TerminalRecoveryBindingConfiguration,
	type TerminalRecoveryLedger,
	type TerminalRecoveryOutcome,
} from './recovery-binding';

export interface TerminalRecoveryCoordinatorDependencies {
	readonly randomCreationId: () => string;
}

export class TerminalRecoveryCoordinator {
	readonly #configuration: TerminalRecoveryBindingConfiguration;
	readonly #dependencies: TerminalRecoveryCoordinatorDependencies;
	readonly #host: HostConnection;
	readonly #ledger: TerminalRecoveryLedger;

	constructor(
		host: HostConnection,
		ledger: TerminalRecoveryLedger,
		configuration: TerminalRecoveryBindingConfiguration,
		dependencies: TerminalRecoveryCoordinatorDependencies,
	) {
		this.#host = host;
		this.#ledger = ledger;
		this.#configuration = {
			bindingId: configuration.bindingId,
			launch: TerminalLaunchV1_1Schema.parse(configuration.launch),
		};
		this.#dependencies = dependencies;
	}

	async reconcile(): Promise<TerminalRecoveryOutcome> {
		const loaded = await this.#ledger.load();
		if (loaded.kind === 'unavailable') {
			return { kind: 'ledger-unavailable', reason: loaded.reason };
		}
		const inventory = await this.#listTerminals();
		if (loaded.kind === 'missing') {
			if (inventory.length > 0) return { kind: 'orphaned', terminalCount: inventory.length };
			const binding: TerminalRecoveryBinding = {
				bindingId: this.#configuration.bindingId,
				creationId: CreationIdSchema.parse(this.#dependencies.randomCreationId()),
				launch: this.#configuration.launch,
				state: { kind: 'planned' },
			};
			if (!(await this.#save(binding))) {
				return { kind: 'ledger-unavailable', reason: 'write-failed' };
			}
			return this.#reconcilePlanned(binding, inventory, true);
		}

		const binding = loaded.document.binding;
		if (
			binding.bindingId !== this.#configuration.bindingId ||
			!launchesEqual(binding.launch, this.#configuration.launch)
		) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		switch (binding.state.kind) {
			case 'planned':
				return this.#reconcilePlanned(binding, inventory, true);
			case 'create-requested':
				return this.#reconcileCreateRequested(binding, binding.state, inventory);
			case 'retained':
			case 'exited':
				return this.#reconcileRetained(binding, binding.state, inventory);
			case 'lost':
				return { kind: 'lost', reason: binding.state.reason };
		}
	}

	async recordExit(event: TerminalExited): Promise<void> {
		const loaded = await this.#ledger.load();
		if (loaded.kind !== 'available') return;
		const binding = loaded.document.binding;
		if (
			binding.state.kind === 'lost' ||
			!identitiesEqual(bindingIdentity(binding), event.identity)
		) {
			return;
		}
		await this.#ledger.save(
			TerminalRecoveryLedgerDocumentSchema.parse({
				binding: {
					...binding,
					state: {
						exit: event.exit,
						finalRevision: event.finalRevision,
						identity: event.identity,
						kind: 'exited',
					},
				},
				version: 1,
			}),
		);
	}

	async #listTerminals(): Promise<readonly TerminalSummary[]> {
		const result = await this.#host.requestControl(
			MessageKind.listTerminals,
			EmptyControlSchema.parse({}),
			MessageKind.listTerminals,
			TerminalListSchema,
		);
		return result.terminals;
	}

	async #reconcilePlanned(
		binding: TerminalRecoveryBinding,
		inventory: readonly TerminalSummary[],
		isProvisioning: boolean,
	): Promise<TerminalRecoveryOutcome> {
		const other = inventory.find((summary) => summary.identity.creationId !== binding.creationId);
		if (other !== undefined) return { kind: 'conflict', reason: 'orphan-conflict' };
		const requested: TerminalRecoveryBinding = {
			...binding,
			state: { hostEpoch: this.#host.hostEpoch, kind: 'create-requested' },
		};
		if (!(await this.#save(requested))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return this.#createAndRetain(requested, isProvisioning);
	}

	async #reconcileCreateRequested(
		binding: TerminalRecoveryBinding,
		state: { readonly hostEpoch: HostEpoch; readonly kind: 'create-requested' },
		inventory: readonly TerminalSummary[],
	): Promise<TerminalRecoveryOutcome> {
		if (state.hostEpoch !== this.#host.hostEpoch) {
			return this.#lose(binding, null, 'host-epoch-changed');
		}
		const match = inventory.find((summary) => summary.identity.creationId === binding.creationId);
		if (match !== undefined) return this.#validateAndRetain(binding, match);
		if (inventory.length > 0) return { kind: 'conflict', reason: 'orphan-conflict' };
		return this.#createAndRetain(binding, false);
	}

	async #reconcileRetained(
		binding: TerminalRecoveryBinding,
		state:
			| { readonly identity: TerminalIdentity; readonly kind: 'retained' }
			| {
					readonly identity: TerminalIdentity;
					readonly kind: 'exited';
					readonly exit: TerminalExited['exit'];
					readonly finalRevision: TerminalExited['finalRevision'];
			  },
		inventory: readonly TerminalSummary[],
	): Promise<TerminalRecoveryOutcome> {
		if (state.identity.hostEpoch !== this.#host.hostEpoch) {
			return this.#lose(binding, state.identity, 'host-epoch-changed');
		}
		const match = inventory.find((summary) => summary.identity.creationId === binding.creationId);
		if (match === undefined) {
			return this.#lose(binding, state.identity, 'identity-missing');
		}
		if (!identitiesEqual(match.identity, state.identity)) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		return this.#validateAndRetain(binding, match);
	}

	async #createAndRetain(
		binding: TerminalRecoveryBinding,
		isProvisioning: boolean,
	): Promise<TerminalRecoveryOutcome> {
		let identity: TerminalIdentity;
		try {
			identity = await this.#host.requestControl(
				MessageKind.createTerminal,
				CreateTerminalV1_1Schema.parse({
					creationId: binding.creationId,
					launch: binding.launch,
				}),
				MessageKind.createTerminal,
				TerminalIdentitySchema,
			);
		} catch (error) {
			if (error instanceof HostRequestError && error.code === 'creation-conflict') {
				return { kind: 'conflict', reason: 'creation-conflict' };
			}
			return { kind: 'lost', reason: 'create-response-unavailable' };
		}
		if (identity.creationId !== binding.creationId || identity.hostEpoch !== this.#host.hostEpoch) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		const retained: TerminalRecoveryBinding = {
			...binding,
			state: { identity, kind: 'retained' },
		};
		if (!(await this.#save(retained))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return isProvisioning
			? { identity, kind: 'provisioning', summary: null }
			: {
					identity,
					kind: 'recovered',
					summary: await this.#requireMatchingSummary(binding.creationId, identity),
				};
	}

	async #validateAndRetain(
		binding: TerminalRecoveryBinding,
		summary: TerminalSummary,
	): Promise<TerminalRecoveryOutcome> {
		let identity: TerminalIdentity;
		try {
			identity = await this.#host.requestControl(
				MessageKind.createTerminal,
				CreateTerminalV1_1Schema.parse({
					creationId: binding.creationId,
					launch: binding.launch,
				}),
				MessageKind.createTerminal,
				TerminalIdentitySchema,
			);
		} catch (error) {
			return error instanceof HostRequestError && error.code === 'creation-conflict'
				? { kind: 'conflict', reason: 'creation-conflict' }
				: { kind: 'lost', reason: 'create-response-unavailable' };
		}
		if (!identitiesEqual(identity, summary.identity)) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		if (summary.phase === TerminalPhase.exited && summary.exit === null) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		const state: TerminalRecoveryBinding['state'] =
			summary.phase === TerminalPhase.exited && summary.exit !== null
				? {
						exit: summary.exit,
						finalRevision: summary.revision,
						identity,
						kind: 'exited',
					}
				: { identity, kind: 'retained' };
		if (!(await this.#save({ ...binding, state }))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return { identity, kind: 'recovered', summary };
	}

	async #requireMatchingSummary(
		creationId: TerminalRecoveryBinding['creationId'],
		identity: TerminalIdentity,
	): Promise<TerminalSummary> {
		const inventory = await this.#listTerminals();
		const match = inventory.find((summary) => summary.identity.creationId === creationId);
		if (match === undefined || !identitiesEqual(match.identity, identity)) {
			throw new Error('Created terminal is absent from Host inventory.');
		}
		return match;
	}

	async #lose(
		binding: TerminalRecoveryBinding,
		previousIdentity: TerminalIdentity | null,
		reason: 'host-epoch-changed' | 'identity-missing',
	): Promise<TerminalRecoveryOutcome> {
		if (!(await this.#save({ ...binding, state: { kind: 'lost', previousIdentity, reason } }))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return { kind: 'lost', reason };
	}

	async #save(binding: TerminalRecoveryBinding): Promise<boolean> {
		try {
			await this.#ledger.save(TerminalRecoveryLedgerDocumentSchema.parse({ binding, version: 1 }));
			return true;
		} catch {
			return false;
		}
	}
}

function bindingIdentity(binding: TerminalRecoveryBinding): TerminalIdentity | null {
	return 'identity' in binding.state ? binding.state.identity : null;
}

function identitiesEqual(left: TerminalIdentity | null, right: TerminalIdentity): boolean {
	return (
		left !== null &&
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function launchesEqual(
	left: TerminalRecoveryBinding['launch'],
	right: TerminalRecoveryBinding['launch'],
): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
