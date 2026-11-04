import { z } from 'zod';

const opaqueIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u);

export const TerminalPresentationIdSchema = opaqueIdentitySchema.brand<'TerminalPresentationId'>();
export type TerminalPresentationId = z.infer<typeof TerminalPresentationIdSchema>;

export const TerminalPresentationOperationIdSchema =
	opaqueIdentitySchema.brand<'TerminalPresentationOperationId'>();
type TerminalPresentationOperationId = z.infer<typeof TerminalPresentationOperationIdSchema>;

export const TerminalPresentationRemovalIdSchema =
	opaqueIdentitySchema.brand<'TerminalPresentationRemovalId'>();
export type TerminalPresentationRemovalId = z.infer<typeof TerminalPresentationRemovalIdSchema>;

type TerminalPresentationFailure = {
	readonly code: string;
	readonly retryable: boolean;
};

export type TerminalPresentationLifecycle =
	| { readonly kind: 'preview' }
	| {
			readonly kind: 'provisioning';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| { readonly kind: 'restoring' }
	| {
			readonly kind: 'reconciling';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| { readonly kind: 'running' }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'provision-failed';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'restore-failed';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| {
			readonly kind: 'stopping';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'stop-failed';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| { readonly kind: 'stopped' }
	| {
			readonly exitCode: number | null;
			readonly kind: 'exited';
			readonly signal: string | null;
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'failed';
	  }
	| {
			readonly kind: 'closing';
			readonly operationId: TerminalPresentationOperationId;
			readonly removalId: TerminalPresentationRemovalId;
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'close-failed';
			readonly operationId: TerminalPresentationOperationId;
			readonly removalId: TerminalPresentationRemovalId;
	  }
	| {
			readonly kind: 'closed';
			readonly operationId: TerminalPresentationOperationId;
			readonly removalId: TerminalPresentationRemovalId;
	  };

/** Transient evidence used only to decide whether closing a live terminal needs confirmation. */
export type TerminalPresentationCloseActivity = 'active' | 'idle' | 'unknown';

export function terminalPresentationId(value: string): TerminalPresentationId {
	return TerminalPresentationIdSchema.parse(value);
}

export function terminalPresentationOperationId(value: string): TerminalPresentationOperationId {
	return TerminalPresentationOperationIdSchema.parse(value);
}
