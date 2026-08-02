import type { TerminalExited } from '@orchbee/terminal-protocol';

import type { TerminalRecoveryOutcome } from '../recovery';
import type { TerminalCloseOutcome, TerminalExitDisposition } from './TerminalCloseOrchestrator';

interface TerminalBindingRecoveryPort {
	reconcile(): Promise<TerminalRecoveryOutcome>;
	recordExit(event: TerminalExited): Promise<void>;
}

interface TerminalBindingClosePort {
	recordExit(event: TerminalExited): Promise<TerminalExitDisposition>;
	requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome>;
}

export type TerminalBindingLifecycleOutcome =
	| Exclude<TerminalRecoveryOutcome, { readonly kind: 'closing' }>
	| { readonly kind: 'close-completed' }
	| {
			readonly kind: 'close-pending';
			readonly reason: 'terminal-not-running' | 'transport-unavailable';
	  };

export class TerminalBindingLifecycle {
	readonly #closing: TerminalBindingClosePort;
	readonly #recovery: TerminalBindingRecoveryPort;
	#operationTail = Promise.resolve();

	constructor(recovery: TerminalBindingRecoveryPort, closing: TerminalBindingClosePort) {
		this.#recovery = recovery;
		this.#closing = closing;
	}

	reconcile(): Promise<TerminalBindingLifecycleOutcome> {
		return this.#enqueue(async () => {
			const outcome = await this.#recovery.reconcile();
			if (outcome.kind !== 'closing') return outcome;
			const closeOutcome = await this.#closing.requestClose(() => Promise.resolve());
			return closeOutcomeToLifecycleOutcome(closeOutcome);
		});
	}

	requestClose(retireProjection: () => Promise<void>): Promise<TerminalCloseOutcome> {
		return this.#enqueue(async () => this.#closing.requestClose(retireProjection));
	}

	recordExit(event: TerminalExited): Promise<void> {
		return this.#enqueue(async () => {
			const disposition = await this.#closing.recordExit(event);
			if (!disposition.handled) await this.#recovery.recordExit(event);
		});
	}

	#enqueue<Output>(operation: () => Promise<Output>): Promise<Output> {
		const result = this.#operationTail.then(operation);
		this.#operationTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}
}

function closeOutcomeToLifecycleOutcome(
	outcome: TerminalCloseOutcome,
): TerminalBindingLifecycleOutcome {
	switch (outcome.kind) {
		case 'closed':
			return { kind: 'close-completed' };
		case 'pending':
			return { kind: 'close-pending', reason: outcome.reason };
		case 'conflict':
			return outcome;
		case 'store-unavailable':
			return { kind: 'ledger-unavailable', reason: outcome.reason };
	}
}
