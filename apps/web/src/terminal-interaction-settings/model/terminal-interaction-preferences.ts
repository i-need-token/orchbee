import {
	DEFAULT_TERMINAL_INTERACTION_CONFIGURATION,
	type TerminalInteractionConfiguration,
} from '@orchbee/terminal';

export interface TerminalInteractionPreferences {
	readonly copyOnSelect: boolean;
	readonly trimTrailingBlankRowsForAgents: boolean;
}

export interface TerminalTrailingBlankRowsPreferenceSource {
	current(): boolean;
	subscribe(listener: (enabled: boolean) => void): () => void;
}

export const DEFAULT_TERMINAL_INTERACTION_PREFERENCES: TerminalInteractionPreferences =
	Object.freeze({
		copyOnSelect: DEFAULT_TERMINAL_INTERACTION_CONFIGURATION.copyOnSelect,
		trimTrailingBlankRowsForAgents: true,
	});

export function terminalInteractionConfigurationFromPreferences(
	preferences: TerminalInteractionPreferences,
): TerminalInteractionConfiguration {
	return Object.freeze({ copyOnSelect: preferences.copyOnSelect });
}
