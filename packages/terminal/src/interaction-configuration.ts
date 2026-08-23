/** Complete client-owned terminal interaction policy changed as one live record. */
export interface TerminalInteractionConfiguration {
	readonly copyOnSelect: boolean;
}

export type TerminalInteractionConfigurationValidationResult =
	| { readonly configuration: TerminalInteractionConfiguration; readonly kind: 'valid' }
	| { readonly field: 'configuration'; readonly kind: 'invalid' };

/** Stable application-owned interaction policy observed by existing projected terminals. */
export interface TerminalInteractionConfigurationSource {
	current(): TerminalInteractionConfiguration;
	subscribe(listener: (configuration: TerminalInteractionConfiguration) => void): () => void;
}

export const DEFAULT_TERMINAL_INTERACTION_CONFIGURATION: TerminalInteractionConfiguration =
	Object.freeze({ copyOnSelect: true });

export const DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE: TerminalInteractionConfigurationSource =
	Object.freeze({
		current: () => DEFAULT_TERMINAL_INTERACTION_CONFIGURATION,
		subscribe: () => () => undefined,
	});

export function normalizeTerminalInteractionConfiguration(
	input: unknown,
): TerminalInteractionConfigurationValidationResult {
	try {
		if (typeof input !== 'object' || input === null || Array.isArray(input)) return invalid();
		const keys = Reflect.ownKeys(input);
		if (keys.length !== 1 || keys[0] !== 'copyOnSelect') return invalid();
		const copyOnSelect = Reflect.get(input, 'copyOnSelect');
		if (typeof copyOnSelect !== 'boolean') return invalid();
		return { configuration: Object.freeze({ copyOnSelect }), kind: 'valid' };
	} catch {
		return invalid();
	}
}

function invalid(): TerminalInteractionConfigurationValidationResult {
	return { field: 'configuration', kind: 'invalid' };
}
