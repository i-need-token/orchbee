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
export type TerminalPresentationOperationId = z.infer<typeof TerminalPresentationOperationIdSchema>;

export const TerminalPresentationRemovalIdSchema =
	opaqueIdentitySchema.brand<'TerminalPresentationRemovalId'>();
export type TerminalPresentationRemovalId = z.infer<typeof TerminalPresentationRemovalIdSchema>;

export const TerminalPresentationFailureSchema = z
	.object({
		code: z.string().min(1).max(128),
		retryable: z.boolean(),
	})
	.readonly();
export type TerminalPresentationFailure = z.infer<typeof TerminalPresentationFailureSchema>;

export type TerminalPresentationLifecycle =
	| { readonly kind: 'preview' }
	| {
			readonly kind: 'provisioning';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| { readonly kind: 'running' }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'provision-failed';
			readonly operationId: TerminalPresentationOperationId;
	  }
	| {
			readonly exitCode: number | null;
			readonly kind: 'exited';
			readonly signal: string | null;
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

export type TerminalPresentationCreation =
	| {
			readonly kind: 'preview';
			readonly presentationId: TerminalPresentationId;
	  }
	| {
			readonly kind: 'managed';
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
	  };

export interface TerminalPresentationCloseOperation {
	readonly operationId: TerminalPresentationOperationId;
	readonly presentationId: TerminalPresentationId;
}

export type TerminalPresentationCloseRequest =
	| { readonly kind: 'preview' }
	| {
			readonly kind: 'managed';
			readonly operations: readonly TerminalPresentationCloseOperation[];
			readonly removalId: TerminalPresentationRemovalId;
	  };

export type TerminalPresentationCommand =
	| {
			readonly launchCwd: string | null;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-provision-requested';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-close-requested';
	  };

export function terminalPresentationId(value: string): TerminalPresentationId {
	return TerminalPresentationIdSchema.parse(value);
}

export function terminalPresentationOperationId(value: string): TerminalPresentationOperationId {
	return TerminalPresentationOperationIdSchema.parse(value);
}

export function terminalPresentationRemovalId(value: string): TerminalPresentationRemovalId {
	return TerminalPresentationRemovalIdSchema.parse(value);
}
