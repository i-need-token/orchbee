import type { TerminalIdentity, TerminalLaunch, TerminalSummary } from '@orchbee/terminal-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';

export type TerminalRecoveryOutcome =
	| {
			readonly identity: TerminalIdentity;
			readonly kind: 'provisioning';
			readonly summary: null;
	  }
	| {
			readonly identity: TerminalIdentity;
			readonly kind: 'recovered';
			readonly summary: TerminalSummary;
	  }
	| {
			readonly identity: TerminalIdentity;
			readonly kind: 'closing';
			readonly summary: TerminalSummary | null;
	  }
	| {
			readonly kind: 'orphaned';
			readonly terminalCount: number;
	  }
	| {
			readonly kind: 'lost';
			readonly reason:
				| 'create-response-unavailable'
				| 'host-epoch-changed'
				| 'identity-conflict'
				| 'identity-missing';
	  }
	| {
			readonly kind: 'conflict';
			readonly reason: 'creation-conflict' | 'identity-conflict' | 'orphan-conflict';
	  }
	| {
			readonly kind: 'ledger-unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

export interface TerminalRecoveryBindingConfiguration {
	readonly bindingId: string;
	readonly launch: TerminalLaunch;
	readonly presentationId: TerminalPresentationId;
}
