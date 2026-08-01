import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	ExitResultSchema,
	HostEpochSchema,
	TerminalIdentitySchema,
	TerminalLaunchV1_1Schema,
	type TerminalIdentity,
	type TerminalLaunchV1_1,
	type TerminalSummary,
} from '@orchbee/terminal-protocol';
import { z } from 'zod';

const bindingIdSchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9._-]+$/u);

const plannedStateSchema = z.strictObject({ kind: z.literal('planned') });
const createRequestedStateSchema = z.strictObject({
	hostEpoch: HostEpochSchema,
	kind: z.literal('create-requested'),
});
const retainedStateSchema = z.strictObject({
	identity: TerminalIdentitySchema,
	kind: z.literal('retained'),
});
const exitedStateSchema = z.strictObject({
	exit: ExitResultSchema,
	finalRevision: CanonicalUint64TextSchema,
	identity: TerminalIdentitySchema,
	kind: z.literal('exited'),
});
const lostStateSchema = z.strictObject({
	kind: z.literal('lost'),
	previousIdentity: TerminalIdentitySchema.nullable(),
	reason: z.enum(['host-epoch-changed', 'identity-conflict', 'identity-missing']),
});

const TerminalRecoveryBindingSchema = z.strictObject({
	bindingId: bindingIdSchema,
	creationId: CreationIdSchema,
	launch: TerminalLaunchV1_1Schema,
	state: z.discriminatedUnion('kind', [
		plannedStateSchema,
		createRequestedStateSchema,
		retainedStateSchema,
		exitedStateSchema,
		lostStateSchema,
	]),
});

export const TerminalRecoveryLedgerDocumentSchema = z.strictObject({
	binding: TerminalRecoveryBindingSchema,
	version: z.literal(1),
});

export type TerminalRecoveryBinding = z.infer<typeof TerminalRecoveryBindingSchema>;
export type TerminalRecoveryLedgerDocument = z.infer<typeof TerminalRecoveryLedgerDocumentSchema>;

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

export type TerminalRecoveryLedgerLoadResult =
	| { readonly kind: 'available'; readonly document: TerminalRecoveryLedgerDocument }
	| { readonly kind: 'missing' }
	| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' };

export interface TerminalRecoveryLedger {
	load(): Promise<TerminalRecoveryLedgerLoadResult>;
	save(document: TerminalRecoveryLedgerDocument): Promise<void>;
}

export interface TerminalRecoveryBindingConfiguration {
	readonly bindingId: string;
	readonly launch: TerminalLaunchV1_1;
}
