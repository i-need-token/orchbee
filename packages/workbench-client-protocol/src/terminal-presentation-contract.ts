import { z } from 'zod';

const opaqueIdentitySchema = z
	.string()
	.min(1)
	.max(128)
	.regex(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/u);

export const TerminalPresentationIdSchema = opaqueIdentitySchema.brand<'TerminalPresentationId'>();
export type TerminalPresentationId = z.infer<typeof TerminalPresentationIdSchema>;

/** Why a terminal ended or never started, in the words of whoever reported it. */
interface TerminalPresentationFailure {
	readonly code: string;
}

/**
 * What a Client knows of the terminal behind one Presentation: it runs, it ended where this Client
 * saw, the Server's launch of it failed, or the Server bound no terminal to it. Nothing retries a
 * failed launch or binds a terminal by itself.
 */
export type TerminalPresentationLifecycle =
	| { readonly kind: 'running' }
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
			readonly failure: TerminalPresentationFailure;
			readonly kind: 'launch-failed';
	  }
	| { readonly kind: 'unbound' };

/** Transient evidence used only to decide whether closing a live terminal needs confirmation. */
export type TerminalPresentationCloseActivity = 'active' | 'idle' | 'unknown';

/** @internal Builds presentation IDs for tests in this package and `apps/web`; production boundaries parse them with `TerminalPresentationIdSchema`. */
export function terminalPresentationId(value: string): TerminalPresentationId {
	return TerminalPresentationIdSchema.parse(value);
}
