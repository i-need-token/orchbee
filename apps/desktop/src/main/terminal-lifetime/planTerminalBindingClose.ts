import {
	TerminalPhase,
	type HostEpoch,
	type TerminalIdentity,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';

import type { TerminalBinding } from '../recovery';

export type TerminalClosePlan =
	| { readonly kind: 'already-closed' }
	| { readonly kind: 'remove-binding' }
	| { readonly kind: 'conflict'; readonly reason: 'identity-conflict' }
	| {
			readonly binding: TerminalBinding;
			readonly identity: TerminalIdentity;
			readonly kind: 'close-terminal';
			readonly summary: TerminalSummary;
	  };

export function planTerminalBindingClose(
	binding: TerminalBinding | undefined,
	inventory: readonly TerminalSummary[],
	hostEpoch: HostEpoch,
): TerminalClosePlan {
	if (binding === undefined) return { kind: 'already-closed' };
	if (binding.state.kind === 'planned' || binding.state.kind === 'lost') {
		return { kind: 'remove-binding' };
	}
	if (binding.state.kind === 'create-requested') {
		if (binding.state.hostEpoch !== hostEpoch) return { kind: 'remove-binding' };
		const match = inventory.find((summary) => summary.identity.creationId === binding.creationId);
		return match === undefined
			? { kind: 'remove-binding' }
			: {
					binding,
					identity: match.identity,
					kind: 'close-terminal',
					summary: match,
				};
	}

	const identity = binding.state.identity;
	if (identity.hostEpoch !== hostEpoch) return { kind: 'remove-binding' };
	const creationMatch = inventory.find(
		(summary) => summary.identity.creationId === identity.creationId,
	);
	const terminalMatch = inventory.find(
		(summary) => summary.identity.terminalId === identity.terminalId,
	);
	if (creationMatch === undefined && terminalMatch === undefined) {
		return { kind: 'remove-binding' };
	}
	if (
		creationMatch === undefined ||
		terminalMatch === undefined ||
		creationMatch !== terminalMatch ||
		!terminalIdentitiesEqual(creationMatch.identity, identity) ||
		(binding.state.kind === 'exited' && creationMatch.phase !== TerminalPhase.exited)
	) {
		return { kind: 'conflict', reason: 'identity-conflict' };
	}
	return {
		binding,
		identity,
		kind: 'close-terminal',
		summary: creationMatch,
	};
}

function terminalIdentitiesEqual(left: TerminalIdentity, right: TerminalIdentity): boolean {
	return (
		left.creationId === right.creationId &&
		left.hostEpoch === right.hostEpoch &&
		left.terminalId === right.terminalId
	);
}
