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
	.strictObject({
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
			readonly launchCwd: string | null;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-restore-requested';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-close-requested';
	  };

export type TerminalPresentationEvent =
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-provision-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-provision-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-restore-started';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-restore-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-restore-failed';
	  }
	| {
			readonly exitCode: number | null;
			readonly presentationId: TerminalPresentationId;
			readonly signal: string | null;
			readonly type: 'terminal-exited';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-failed';
	  }
	| {
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-close-completed';
	  }
	| {
			readonly failure: TerminalPresentationFailure;
			readonly operationId: TerminalPresentationOperationId;
			readonly presentationId: TerminalPresentationId;
			readonly type: 'terminal-close-failed';
	  };

const terminalPresentationOperationIdentitySchema = {
	operationId: TerminalPresentationOperationIdSchema,
	presentationId: TerminalPresentationIdSchema,
} as const;

const terminalPresentationExitSchema = z
	.strictObject({
		exitCode: z.number().int().min(0).max(0xffff_ffff).nullable(),
		presentationId: TerminalPresentationIdSchema,
		signal: z.string().min(1).max(128).nullable(),
		type: z.literal('terminal-exited'),
	})
	.refine((event) => event.exitCode === null || event.signal === null, {
		message: 'A terminal exit cannot contain both an exit code and a signal',
	});

export const TerminalPresentationEventSchema = z.discriminatedUnion('type', [
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		type: z.literal('terminal-provision-completed'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-provision-failed'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		type: z.literal('terminal-restore-started'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		type: z.literal('terminal-restore-completed'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-restore-failed'),
	}),
	z.strictObject({
		failure: TerminalPresentationFailureSchema,
		presentationId: TerminalPresentationIdSchema,
		type: z.literal('terminal-failed'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		type: z.literal('terminal-close-completed'),
	}),
	z.strictObject({
		...terminalPresentationOperationIdentitySchema,
		failure: TerminalPresentationFailureSchema,
		type: z.literal('terminal-close-failed'),
	}),
	terminalPresentationExitSchema,
]) satisfies z.ZodType<TerminalPresentationEvent>;

export function terminalPresentationId(value: string): TerminalPresentationId {
	return TerminalPresentationIdSchema.parse(value);
}

export function terminalPresentationOperationId(value: string): TerminalPresentationOperationId {
	return TerminalPresentationOperationIdSchema.parse(value);
}

export function terminalPresentationRemovalId(value: string): TerminalPresentationRemovalId {
	return TerminalPresentationRemovalIdSchema.parse(value);
}
