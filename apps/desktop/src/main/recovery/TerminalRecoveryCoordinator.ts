import {
	CreateTerminalSchema,
	CreationIdSchema,
	MessageKind,
	TerminalIdentitySchema,
	TerminalLaunchSchema,
	TerminalPhase,
	type HostEpoch,
	type TerminalExited,
	type TerminalIdentity,
	type TerminalLaunch,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import { HostRequestError, readTerminalInventory, type HostConnection } from '../host';
import type {
	TerminalRecoveryBindingConfiguration,
	TerminalRecoveryOutcome,
} from './recovery-binding';
import {
	createTerminalBindingStoreDocument,
	findTerminalBinding,
	TerminalBindingIdSchema,
	terminalBindingIdentity,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStore,
	type TerminalBindingStoreDocument,
} from './terminal-binding-store';

export interface TerminalRecoveryCoordinatorDependencies {
	readonly randomCreationId: () => string;
	readonly onIdentityObserved?: (identity: TerminalIdentity) => void;
}

export type TerminalExitPersistenceResult =
	| { readonly durable: true }
	| {
			readonly durable: false;
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

export class TerminalRecoveryCoordinator {
	readonly #bindingId: TerminalBindingId;
	readonly #dependencies: TerminalRecoveryCoordinatorDependencies;
	readonly #host: HostConnection;
	readonly #launch: TerminalLaunch;
	readonly #presentationId: TerminalPresentationId;
	readonly #store: TerminalBindingStore;
	#associatedIdentity: TerminalIdentity | null = null;

	constructor(
		host: HostConnection,
		store: TerminalBindingStore,
		configuration: TerminalRecoveryBindingConfiguration,
		dependencies: TerminalRecoveryCoordinatorDependencies,
	) {
		this.#host = host;
		this.#store = store;
		this.#bindingId = TerminalBindingIdSchema.parse(configuration.bindingId);
		this.#launch = TerminalLaunchSchema.parse(configuration.launch);
		this.#presentationId = TerminalPresentationIdSchema.parse(configuration.presentationId);
		this.#dependencies = dependencies;
	}

	async reconcile(): Promise<TerminalRecoveryOutcome> {
		const loaded = await this.#store.load();
		if (loaded.kind === 'unavailable') {
			return { kind: 'ledger-unavailable', reason: loaded.reason };
		}
		const document =
			loaded.kind === 'available' ? loaded.document : createTerminalBindingStoreDocument();
		const inventory = await readTerminalInventory(this.#host);
		const binding = findTerminalBinding(document, this.#bindingId);
		if (binding === undefined) {
			const unboundCount = countUnboundTerminals(document, inventory);
			if (unboundCount > 0) return { kind: 'orphaned', terminalCount: unboundCount };
			const planned: TerminalBinding = {
				bindingId: this.#bindingId,
				creationId: CreationIdSchema.parse(this.#dependencies.randomCreationId()),
				launch: this.#launch,
				presentationId: this.#presentationId,
				state: { kind: 'planned' },
			};
			if (!(await this.#save(planned))) {
				return { kind: 'ledger-unavailable', reason: 'write-failed' };
			}
			return this.#reconcilePlanned(planned, true);
		}

		if (binding.state.kind === 'closing') {
			return reconcileClosing(binding, binding.state, inventory, this.#host.hostEpoch);
		}
		if (
			binding.presentationId !== this.#presentationId ||
			!launchesEqual(binding.launch, this.#launch)
		) {
			return { kind: 'conflict', reason: 'identity-conflict' };
		}
		if (binding.state.kind !== 'lost' && countUnboundTerminals(document, inventory) > 0) {
			return { kind: 'conflict', reason: 'orphan-conflict' };
		}
		switch (binding.state.kind) {
			case 'planned':
				return this.#reconcilePlanned(binding, true);
			case 'create-requested':
				return this.#reconcileCreateRequested(binding, binding.state, inventory);
			case 'retained':
			case 'exited':
				return this.#reconcileRetained(binding, binding.state, inventory);
			case 'lost':
				return { kind: 'lost', reason: binding.state.reason };
		}
	}

	async recordExit(event: TerminalExited): Promise<TerminalExitPersistenceResult | void> {
		const loaded = await this.#store.load();
		if (loaded.kind === 'unavailable') {
			return { durable: false, reason: loaded.reason };
		}
		if (loaded.kind === 'missing') return { durable: true };
		const binding = findTerminalBinding(loaded.document, this.#bindingId);
		let bindingIdentity: TerminalIdentity | null;
		if (binding === undefined) {
			bindingIdentity = null;
		} else if (binding.state.kind === 'create-requested') {
			const associatedIdentity = this.#associatedIdentity;
			bindingIdentity =
				associatedIdentity !== null &&
				associatedIdentity.creationId === binding.creationId &&
				associatedIdentity.hostEpoch === binding.state.hostEpoch
					? associatedIdentity
					: null;
		} else {
			bindingIdentity = terminalBindingIdentity(binding);
		}
		if (
			binding === undefined ||
			binding.state.kind === 'closing' ||
			binding.state.kind === 'lost' ||
			!identitiesEqual(bindingIdentity, event.identity)
		) {
			return { durable: true };
		}
		if (
			binding.state.kind === 'exited' &&
			BigInt(event.finalRevision) <= BigInt(binding.state.finalRevision)
		) {
			return { durable: true };
		}
		try {
			await this.#store.put({
				...binding,
				state: {
					exit: event.exit,
					finalRevision: event.finalRevision,
					identity: event.identity,
					kind: 'exited',
				},
			});
			return { durable: true };
		} catch {
			return { durable: false, reason: 'write-failed' };
		}
	}

	async #reconcilePlanned(
		binding: TerminalBinding,
		isProvisioning: boolean,
	): Promise<TerminalRecoveryOutcome> {
		const requested: TerminalBinding = {
			...binding,
			state: { hostEpoch: this.#host.hostEpoch, kind: 'create-requested' },
		};
		if (!(await this.#save(requested))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return this.#createAndRetain(requested, isProvisioning);
	}

	async #reconcileCreateRequested(
		binding: TerminalBinding,
		state: { readonly hostEpoch: HostEpoch; readonly kind: 'create-requested' },
		inventory: readonly TerminalSummary[],
	): Promise<TerminalRecoveryOutcome> {
		if (state.hostEpoch !== this.#host.hostEpoch) {
			return this.#lose(binding, null, 'host-epoch-changed');
		}
		const match = inventory.find((summary) => summary.identity.creationId === binding.creationId);
		if (match !== undefined) return this.#validateAndRetain(binding, match);
		return this.#createAndRetain(binding, false);
	}

	async #reconcileRetained(
		binding: TerminalBinding,
		state:
			| { readonly identity: TerminalIdentity; readonly kind: 'retained' }
			| {
					readonly exit: TerminalExited['exit'];
					readonly finalRevision: TerminalExited['finalRevision'];
					readonly identity: TerminalIdentity;
					readonly kind: 'exited';
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
		binding: TerminalBinding,
		isProvisioning: boolean,
	): Promise<TerminalRecoveryOutcome> {
		let identity: TerminalIdentity;
		try {
			identity = await this.#host.requestControl(
				MessageKind.createTerminal,
				CreateTerminalSchema.parse({
					creationId: binding.creationId,
					launch: binding.launch,
				}),
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
		this.#observeIdentity(identity);
		const retained: TerminalBinding = {
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
		binding: TerminalBinding,
		summary: TerminalSummary,
	): Promise<TerminalRecoveryOutcome> {
		let identity: TerminalIdentity;
		try {
			identity = await this.#host.requestControl(
				MessageKind.createTerminal,
				CreateTerminalSchema.parse({
					creationId: binding.creationId,
					launch: binding.launch,
				}),
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
		this.#observeIdentity(identity);
		const state: TerminalBinding['state'] =
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
		creationId: TerminalBinding['creationId'],
		identity: TerminalIdentity,
	): Promise<TerminalSummary> {
		const inventory = await readTerminalInventory(this.#host);
		const match = inventory.find((summary) => summary.identity.creationId === creationId);
		if (match === undefined || !identitiesEqual(match.identity, identity)) {
			throw new Error('Created terminal is absent from Host inventory.');
		}
		return match;
	}

	async #lose(
		binding: TerminalBinding,
		previousIdentity: TerminalIdentity | null,
		reason: 'host-epoch-changed' | 'identity-missing',
	): Promise<TerminalRecoveryOutcome> {
		if (!(await this.#save({ ...binding, state: { kind: 'lost', previousIdentity, reason } }))) {
			return { kind: 'ledger-unavailable', reason: 'write-failed' };
		}
		return { kind: 'lost', reason };
	}

	async #save(binding: TerminalBinding): Promise<boolean> {
		try {
			await this.#store.put(binding);
			return true;
		} catch {
			return false;
		}
	}

	#observeIdentity(identity: TerminalIdentity): void {
		if (this.#associatedIdentity !== null && !identitiesEqual(this.#associatedIdentity, identity)) {
			throw new Error('Terminal Recovery Coordinator observed a conflicting Host identity.');
		}
		this.#associatedIdentity = identity;
		this.#dependencies.onIdentityObserved?.(identity);
	}
}

function countUnboundTerminals(
	document: TerminalBindingStoreDocument,
	inventory: readonly TerminalSummary[],
): number {
	return inventory.filter(
		(summary) => !document.bindings.some((binding) => bindingOwnsSummary(binding, summary)),
	).length;
}

function bindingOwnsSummary(binding: TerminalBinding, summary: TerminalSummary): boolean {
	if (binding.creationId !== summary.identity.creationId) return false;
	switch (binding.state.kind) {
		case 'create-requested':
			return binding.state.hostEpoch === summary.identity.hostEpoch;
		case 'retained':
		case 'exited':
		case 'closing':
			return identitiesEqual(binding.state.identity, summary.identity);
		case 'lost':
		case 'planned':
			return false;
	}
}

function reconcileClosing(
	binding: TerminalBinding,
	state: Extract<TerminalBinding['state'], { readonly kind: 'closing' }>,
	inventory: readonly TerminalSummary[],
	hostEpoch: HostEpoch,
): TerminalRecoveryOutcome {
	const identity = state.identity;
	if (identity.hostEpoch !== hostEpoch) {
		return { disposition: state.disposition, identity, kind: 'closing', summary: null };
	}
	const match = inventory.find((summary) => summary.identity.creationId === binding.creationId);
	if (match === undefined) {
		return { disposition: state.disposition, identity, kind: 'closing', summary: null };
	}
	return identitiesEqual(identity, match.identity)
		? { disposition: state.disposition, identity, kind: 'closing', summary: match }
		: { kind: 'conflict', reason: 'identity-conflict' };
}

function identitiesEqual(left: TerminalIdentity | null, right: TerminalIdentity): boolean {
	return (
		left !== null &&
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function launchesEqual(left: TerminalBinding['launch'], right: TerminalBinding['launch']): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
