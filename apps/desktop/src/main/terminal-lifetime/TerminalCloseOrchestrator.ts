import {
	decodeControlJson,
	MessageKind,
	TerminalAcknowledgedSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TerminalPhase,
	TerminalTargetSchema,
	type DecodedProtocolEnvelope,
	type TerminalExited,
	type TerminalFailed,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';

import { HostRequestError, readTerminalInventory, type HostConnection } from '../host';
import {
	findTerminalBinding,
	TerminalBindingIdSchema,
	type TerminalBinding,
	type TerminalBindingId,
	type TerminalBindingStore,
} from '../recovery';
import { planTerminalBindingClose, type TerminalClosePlan } from './planTerminalBindingClose';

const CLOSE_GRACE_MILLISECONDS = 5_000;

export type TerminalCloseOutcome =
	| { readonly kind: 'closed' }
	| { readonly kind: 'conflict'; readonly reason: 'identity-conflict' }
	| {
			readonly kind: 'pending';
			readonly reason: 'terminal-not-running' | 'transport-unavailable';
	  }
	| {
			readonly kind: 'store-unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

export interface TerminalCloseOrchestratorDependencies {
	readonly waitForGracePeriod?: () => Promise<void>;
}

export interface TerminalExitDisposition {
	readonly handled: boolean;
	/** Present and false when exit persistence was unavailable after close routing declined it. */
	readonly durable?: boolean;
	readonly outcome?: TerminalCloseOutcome;
}

export class TerminalCloseOrchestrator {
	readonly #bindingId: TerminalBindingId;
	readonly #host: HostConnection;
	readonly #store: TerminalBindingStore;
	readonly #waitForGracePeriod: () => Promise<void>;

	constructor(
		host: HostConnection,
		store: TerminalBindingStore,
		bindingId: string,
		dependencies: TerminalCloseOrchestratorDependencies = {},
	) {
		this.#host = host;
		this.#store = store;
		this.#bindingId = TerminalBindingIdSchema.parse(bindingId);
		this.#waitForGracePeriod = dependencies.waitForGracePeriod ?? waitForDefaultGracePeriod;
	}

	async requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome> {
		const loaded = await this.#store.load();
		if (loaded.kind === 'unavailable') {
			return { kind: 'store-unavailable', reason: loaded.reason };
		}
		const binding =
			loaded.kind === 'available'
				? findTerminalBinding(loaded.document, this.#bindingId)
				: undefined;
		if (binding === undefined) return { kind: 'closed' };
		if (binding.state.kind === 'planned' || binding.state.kind === 'lost') {
			return this.#removeBinding();
		}

		const outcomes = new TerminalOutcomeObservation(this.#host, binding.creationId);
		const closure = new HostCloseObservation(this.#host);
		try {
			if (binding.state.kind === 'create-requested') {
				const inventory = await readTerminalInventory(this.#host);
				const plan = planTerminalBindingClose(binding, inventory, this.#host.hostEpoch);
				if (plan.kind !== 'close-terminal') return this.#finishNonTerminalPlan(plan);
				if (!(await this.#persistClosing(plan.binding, plan.identity))) {
					return { kind: 'store-unavailable', reason: 'write-failed' };
				}
				await retireProjection();
				const outcome = await this.#driveTerminalClose(
					{ ...plan, binding: toClosingBinding(plan.binding, plan.identity) },
					outcomes,
					closure,
				);
				return outcome;
			}

			const closingBinding = toClosingBinding(binding, binding.state.identity);
			if (binding.state.kind !== 'closing') {
				if (!(await this.#persistClosing(binding, binding.state.identity))) {
					return { kind: 'store-unavailable', reason: 'write-failed' };
				}
			}
			await retireProjection();
			const inventory = await readTerminalInventory(this.#host);
			const plan = planTerminalBindingClose(closingBinding, inventory, this.#host.hostEpoch);
			if (plan.kind !== 'close-terminal') return this.#finishNonTerminalPlan(plan);
			const outcome = await this.#driveTerminalClose(plan, outcomes, closure);
			return outcome;
		} catch (error) {
			await Promise.resolve();
			if (closure.closed) return { kind: 'pending', reason: 'transport-unavailable' };
			throw error;
		} finally {
			outcomes.dispose();
			closure.dispose();
		}
	}

	async recordExit(event: TerminalExited): Promise<TerminalExitDisposition> {
		const loaded = await this.#store.load();
		if (loaded.kind === 'unavailable') {
			return {
				handled: true,
				durable: false,
				outcome: { kind: 'store-unavailable', reason: loaded.reason },
			};
		}
		if (loaded.kind === 'missing') return { handled: false };
		const binding = findTerminalBinding(loaded.document, this.#bindingId);
		if (
			binding?.state.kind !== 'closing' ||
			!identitiesEqual(binding.state.identity, event.identity)
		) {
			return { handled: false };
		}
		const closure = new HostCloseObservation(this.#host);
		try {
			const outcome = await this.#disposeTerminal(event.identity, closure);
			const durable = outcome.kind !== 'pending' && outcome.kind !== 'store-unavailable';
			return {
				handled: true,
				...(durable ? {} : { durable: false }),
				outcome,
			};
		} finally {
			closure.dispose();
		}
	}

	async #driveTerminalClose(
		plan: Extract<TerminalClosePlan, { readonly kind: 'close-terminal' }>,
		outcomes: TerminalOutcomeObservation,
		closure: HostCloseObservation,
		refreshCount = 0,
	): Promise<TerminalCloseOutcome> {
		if (
			outcomes.has(plan.identity) ||
			plan.summary.phase === TerminalPhase.exited ||
			plan.summary.phase === TerminalPhase.failed
		) {
			return this.#disposeTerminal(plan.identity, closure);
		}
		if (plan.summary.phase === TerminalPhase.finishing && !plan.summary.terminateRequested) {
			return { kind: 'pending', reason: 'terminal-not-running' };
		}

		if (plan.summary.phase === TerminalPhase.running) {
			try {
				await this.#requestTerminalLifecycle(MessageKind.terminateTerminal, plan.identity);
			} catch (error) {
				return this.#recoverRejectedMutation(error, plan.binding, outcomes, closure, refreshCount);
			}
			if (outcomes.has(plan.identity)) return this.#disposeTerminal(plan.identity, closure);
		}

		if (plan.summary.phase !== TerminalPhase.containingFailure) {
			const graceResult = await Promise.race([
				outcomes.waitFor(plan.identity).then(() => 'outcome' as const),
				closure.wait().then(() => 'closed' as const),
				this.#waitForGracePeriod().then(() => 'grace' as const),
			]);
			if (graceResult === 'outcome') return this.#disposeTerminal(plan.identity, closure);
			if (graceResult === 'closed') {
				return { kind: 'pending', reason: 'transport-unavailable' };
			}
		}

		try {
			await this.#requestTerminalLifecycle(MessageKind.forceTerminateTerminal, plan.identity);
		} catch (error) {
			return this.#recoverRejectedMutation(error, plan.binding, outcomes, closure, refreshCount);
		}
		if (outcomes.has(plan.identity)) return this.#disposeTerminal(plan.identity, closure);
		const outcomeResult = await Promise.race([
			outcomes.waitFor(plan.identity).then(() => 'outcome' as const),
			closure.wait().then(() => 'closed' as const),
		]);
		return outcomeResult === 'outcome'
			? this.#disposeTerminal(plan.identity, closure)
			: { kind: 'pending', reason: 'transport-unavailable' };
	}

	async #recoverRejectedMutation(
		error: unknown,
		binding: TerminalBinding,
		outcomes: TerminalOutcomeObservation,
		closure: HostCloseObservation,
		refreshCount: number,
	): Promise<TerminalCloseOutcome> {
		await Promise.resolve();
		if (closure.closed) return { kind: 'pending', reason: 'transport-unavailable' };
		if (
			!(error instanceof HostRequestError) ||
			(error.code !== 'invalid-phase' && error.code !== 'missing-resource')
		) {
			throw error;
		}
		if (refreshCount >= 2) throw new Error('Terminal close state did not converge after refresh.');
		const inventory = await readTerminalInventory(this.#host);
		const plan = planTerminalBindingClose(binding, inventory, this.#host.hostEpoch);
		if (plan.kind !== 'close-terminal') return this.#finishNonTerminalPlan(plan);
		return this.#driveTerminalClose(plan, outcomes, closure, refreshCount + 1);
	}

	async #disposeTerminal(
		identity: TerminalIdentity,
		closure: HostCloseObservation,
	): Promise<TerminalCloseOutcome> {
		try {
			await this.#requestTerminalLifecycle(MessageKind.disposeTerminal, identity);
		} catch (error) {
			await Promise.resolve();
			if (closure.closed) return { kind: 'pending', reason: 'transport-unavailable' };
			throw error;
		}
		return this.#removeBinding();
	}

	async #requestTerminalLifecycle(
		kind:
			| typeof MessageKind.disposeTerminal
			| typeof MessageKind.forceTerminateTerminal
			| typeof MessageKind.terminateTerminal,
		identity: TerminalIdentity,
	): Promise<void> {
		const acknowledged = await this.#host.requestControl(
			kind,
			TerminalTargetSchema.parse({ terminalId: identity.terminalId }),
			TerminalAcknowledgedSchema,
		);
		if (acknowledged.terminalId !== identity.terminalId) {
			throw new Error('Terminal Host acknowledged a foreign terminal identity.');
		}
	}

	async #persistClosing(binding: TerminalBinding, identity: TerminalIdentity): Promise<boolean> {
		try {
			await this.#store.put(toClosingBinding(binding, identity));
			return true;
		} catch {
			return false;
		}
	}

	async #removeBinding(): Promise<TerminalCloseOutcome> {
		try {
			await this.#store.remove(this.#bindingId);
			return { kind: 'closed' };
		} catch {
			return { kind: 'store-unavailable', reason: 'write-failed' };
		}
	}

	#finishNonTerminalPlan(plan: TerminalClosePlan): Promise<TerminalCloseOutcome> {
		switch (plan.kind) {
			case 'already-closed':
				return Promise.resolve({ kind: 'closed' });
			case 'remove-binding':
				return this.#removeBinding();
			case 'conflict':
				return Promise.resolve(plan);
			case 'close-terminal':
				throw new Error('A terminal close plan must enter the close state machine.');
		}
	}
}

class HostCloseObservation {
	#closedReason: Error | undefined;
	readonly #promise: Promise<Error>;
	readonly #removeListener: () => void;

	constructor(host: HostConnection) {
		let resolveClose: (reason: Error) => void = () => undefined;
		this.#promise = new Promise((resolve) => {
			resolveClose = resolve;
		});
		this.#removeListener = host.onClosed((reason) => {
			this.#closedReason = reason;
			resolveClose(reason);
		});
	}

	get closed(): boolean {
		return this.#closedReason !== undefined;
	}

	dispose(): void {
		this.#removeListener();
	}

	wait(): Promise<Error> {
		return this.#promise;
	}
}

type TerminalOutcome = TerminalExited | TerminalFailed;

class TerminalOutcomeObservation {
	readonly #events = new Map<string, TerminalOutcome>();
	readonly #removeListener: () => void;
	readonly #targetCreationId: TerminalIdentity['creationId'];
	readonly #waiters = new Map<string, (event: TerminalOutcome) => void>();

	constructor(host: HostConnection, targetCreationId: TerminalIdentity['creationId']) {
		this.#targetCreationId = targetCreationId;
		this.#removeListener = host.onEnvelope((envelope) => this.#handleEnvelope(envelope));
	}

	dispose(): void {
		this.#removeListener();
		this.#waiters.clear();
	}

	has(identity: TerminalIdentity): boolean {
		const event = this.#events.get(identityKey(identity));
		return event !== undefined && identitiesEqual(event.identity, identity);
	}

	waitFor(identity: TerminalIdentity): Promise<TerminalOutcome> {
		const key = identityKey(identity);
		const existing = this.#events.get(key);
		if (existing !== undefined && identitiesEqual(existing.identity, identity)) {
			return Promise.resolve(existing);
		}
		return new Promise((resolve) => this.#waiters.set(key, resolve));
	}

	#handleEnvelope(envelope: DecodedProtocolEnvelope): void {
		if (envelope.kind === MessageKind.terminalExited) {
			const decoded = decodeControlJson(envelope.payload, TerminalExitedSchema);
			if (decoded.ok) this.#recordOutcome(decoded.value);
			return;
		}
		if (envelope.kind === MessageKind.terminalFailed) {
			const decoded = decodeControlJson(envelope.payload, TerminalFailedSchema);
			if (decoded.ok) this.#recordOutcome(decoded.value);
		}
	}

	#recordOutcome(outcome: TerminalOutcome): void {
		if (outcome.identity.creationId !== this.#targetCreationId) return;
		const key = identityKey(outcome.identity);
		this.#events.set(key, outcome);
		this.#waiters.get(key)?.(outcome);
		this.#waiters.delete(key);
	}
}

function toClosingBinding(binding: TerminalBinding, identity: TerminalIdentity): TerminalBinding {
	return { ...binding, state: { identity, kind: 'closing' } };
}

function identitiesEqual(left: TerminalIdentity, right: TerminalIdentity): boolean {
	return (
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}

function identityKey(identity: TerminalIdentity): string {
	return `${identity.hostEpoch}:${identity.terminalId}:${identity.creationId}`;
}

function waitForDefaultGracePeriod(): Promise<void> {
	return new Promise((resolve) => {
		const timeout = setTimeout(resolve, CLOSE_GRACE_MILLISECONDS);
		timeout.unref();
	});
}
