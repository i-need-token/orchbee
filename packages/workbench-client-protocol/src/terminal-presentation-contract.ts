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

export type TerminalPresentationFailure = {
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
			readonly disposition: 'remove' | 'stop';
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

export function terminalPresentationId(value: string): TerminalPresentationId {
	return TerminalPresentationIdSchema.parse(value);
}

export function terminalPresentationOperationId(value: string): TerminalPresentationOperationId {
	return TerminalPresentationOperationIdSchema.parse(value);
}

export function terminalPresentationRemovalId(value: string): TerminalPresentationRemovalId {
	return TerminalPresentationRemovalIdSchema.parse(value);
}
