import type { TerminalResolvedThemePreferences } from './terminal-appearance-preferences';

/** Already prepared terminal publication that is infallible after its synchronous preflight. */
export interface TerminalThemePresentationCommit {
	abort(): void;
	canCommit(): boolean;
	commit(): void;
}

export type TerminalThemePresentationPreparation =
	| { readonly commit: TerminalThemePresentationCommit; readonly kind: 'prepared' }
	| { readonly code: string; readonly kind: 'failed'; readonly retryable: boolean }
	| { readonly kind: 'cancelled' };

/** One mounted projected terminal participating in the application-wide theme barrier. */
export interface TerminalThemePresentationParticipant {
	prepareThemePresentation(
		preferences: TerminalResolvedThemePreferences,
	): Promise<TerminalThemePresentationPreparation>;
}

export interface TerminalThemePresentationSource {
	registerThemeParticipant(participant: TerminalThemePresentationParticipant): () => void;
}

export function isTerminalThemePresentationSource(
	value: unknown,
): value is TerminalThemePresentationSource {
	return (
		typeof value === 'object' &&
		value !== null &&
		typeof Reflect.get(value, 'registerThemeParticipant') === 'function'
	);
}
